package com.tokera.ate.io.core;

import com.google.common.cache.Cache;
import com.google.common.cache.CacheBuilder;
import com.tokera.ate.dao.IRights;
import com.tokera.ate.dao.TopicAndPartition;
import com.tokera.ate.delegates.AteDelegate;
import com.tokera.ate.dto.PrivateKeyWithSeedDto;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.io.api.IPartitionKeyProvider;
import com.tokera.ate.io.repo.*;
import com.tokera.ate.io.task.Task;
import com.tokera.ate.scopes.Startup;
import com.tokera.ate.units.Alias;
import com.tokera.ate.units.DaoId;
import org.apache.commons.lang3.time.DateUtils;
import org.jboss.weld.context.bound.BoundRequestContext;

import javax.enterprise.context.ApplicationScoped;
import javax.enterprise.inject.spi.CDI;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.stream.Collectors;

@Startup
@ApplicationScoped
public class DataMaintenance extends DataPartitionDaemon {
    private final ConcurrentHashMap<TopicAndPartition, State> states;
    private final Random rand = new Random();
    private final AtomicInteger ticks = new AtomicInteger(0);

    public DataMaintenance() {
        this.states = new ConcurrentHashMap<>();
    }

    @Override
    public void removePartition(TopicAndPartition partition) {
        super.removePartition(partition);
        states.remove(partition);
    }

    public class State implements IRights, IPartitionKeyProvider {
        protected final UUID id;
        protected final IPartitionKey key;
        protected final TopicAndPartition what;
        protected final ConcurrentMap<String, Date> tombstones;
        protected final ConcurrentMap<UUID, Date> merges;
        protected final Cache<String, PrivateKeyWithSeedDto> borrowedReadKeys;
        protected final Cache<String, PrivateKeyWithSeedDto> borrowedWriteKeys;

        protected State(IPartitionKey key) {
            this.id = UUID.randomUUID();
            this.key = key;
            this.what = new TopicAndPartition(key);
            this.tombstones = new ConcurrentHashMap<>();

            this.merges = new ConcurrentHashMap<>();
            this.borrowedReadKeys = CacheBuilder.newBuilder()
                    .expireAfterWrite(d.bootstrapConfig.getDataMaintenanceWindow() * 2, TimeUnit.MILLISECONDS)
                    .build();
            this.borrowedWriteKeys = CacheBuilder.newBuilder()
                    .expireAfterWrite(d.bootstrapConfig.getDataMaintenanceWindow() * 2, TimeUnit.MILLISECONDS)
                    .build();
        }

        public void tombstone(String key) {
            int maintenanceWindow = d.bootstrapConfig.getDataMaintenanceWindow();
            this.tombstones.putIfAbsent(key, DateUtils.addMilliseconds(new Date(), maintenanceWindow));
        }

        public void dont_tombstone(String key) {
            this.tombstones.remove(key);
        }

        public void merge(UUID key) {
            int maintenanceWindow = d.bootstrapConfig.getDataMaintenanceWindow();
            this.merges.putIfAbsent(key, DateUtils.addMilliseconds(new Date(), maintenanceWindow));
        }

        public void dont_merge(UUID key) {
            this.merges.remove(key);
        }

        public void lend_rights(IRights rights) {
            for (PrivateKeyWithSeedDto right : rights.getRightsRead()) {
                this.borrowedReadKeys.put(right.publicHash(), right);
            }
            for (PrivateKeyWithSeedDto right : rights.getRightsWrite()) {
                this.borrowedWriteKeys.put(right.publicHash(), right);
            }
        }

        protected List<String> pollTombstones() {
            List<String> ret = new ArrayList<>();

            Date now = new Date();
            for (Map.Entry<String, Date> pair : tombstones.entrySet().stream().collect(Collectors.toSet())) {
                if (now.after(pair.getValue())) {
                    ret.add(pair.getKey());
                    tombstones.remove(pair.getKey());
                }
            }
            return ret;
        }

        protected List<UUID> pollMerges() {
            List<UUID> ret = new ArrayList<>();

            Date now = new Date();
            for (Map.Entry<UUID, Date> pair : merges.entrySet().stream().collect(Collectors.toSet())) {
                if (now.after(pair.getValue())) {
                    ret.add(pair.getKey());
                    tombstones.remove(pair.getKey());
                }
            }
            return ret;
        }

        @Override
        public @DaoId UUID getId() {
            return this.id;
        }

        @Override
        public Set<PrivateKeyWithSeedDto> getRightsRead() {
            return this.borrowedReadKeys.asMap().values().stream().collect(Collectors.toSet());
        }

        @Override
        public Set<PrivateKeyWithSeedDto> getRightsWrite() {
            return this.borrowedWriteKeys.asMap().values().stream().collect(Collectors.toSet());
        }

        @Override
        public @Alias String getRightsAlias() {
            return "maintenance:" + key.toString();
        }

        @Override
        public IPartitionKey partitionKey(boolean shouldThrow) {
            return this.key;
        }
    }

    public State getOrCreateState(IPartitionKey key) {
        TopicAndPartition tp = new TopicAndPartition(key);
        return states.computeIfAbsent(tp, k -> new State(key));
    }

    public void lend_rights(IPartitionKey key, IRights rights) {
        State state = getOrCreateState(key);
        state.lend_rights(rights);
    }

    @Override
    protected void work() throws InterruptedException
    {
        // Get the subscriber and compute the next data merging date point
        DataSubscriber subscriber = AteDelegate.get().storageFactory.get().backend();

        // Loop through all the partitions and do the work on them
        for (TopicAndPartition what : listPartitions()) {
            State state = states.getOrDefault(what, null);
            if (state == null) continue;

            // Add a exception handler
            try {
                // Get the bridge and chain
                DataPartition partition = subscriber.getPartition(what, false);
                if (partition == null) continue;
                IDataPartitionBridge bridge = partition.getBridge();
                if (bridge.hasLoaded() == false) continue;

                // Delete anything that should be tomb-stoned
                List<String> tombstones = state.pollTombstones();
                if (tombstones.size() > 0) {
                    bridge.deleteMany(tombstones);
                }

                // Merge everything that needs merging
                List<UUID> toMerge = state.pollMerges();
                if (toMerge.size() > 0)
                {
                    // Create the bounded request context
                    BoundRequestContext boundRequestContext = CDI.current().select(BoundRequestContext.class).get();
                    Task.enterRequestScopeAndInvoke(state.key, boundRequestContext, null, () -> {
                        d.currentRights.impersonate(state);

                        for (UUID id : toMerge) {
                            try {
                                performMerge(partition, id);
                            } catch (Throwable ex) {
                                if (ex instanceof InterruptedException) throw ex;
                                this.LOG.warn(ex);
                            }
                        }
                    });
                }
            } catch (Throwable ex) {
                if (ex instanceof InterruptedException) throw ex;
                this.LOG.warn(ex);
            }
        }

        // Increment the tick count
        ticks.incrementAndGet();

        // Wait a second (or more) - the random wait time helps reduce merge thrashing
        int waitTime = 1000 + rand.nextInt(4000);
        Thread.sleep(waitTime);
    }

    private void performMerge(DataPartition partition, UUID id)
    {
        IDataPartitionBridge bridge = partition.getBridge();
        if (bridge.hasLoaded() == false) return;
        DataPartitionChain chain = partition.getChain(false);

        // First get the container and check if it still actually needs a merge
        DataContainer container = chain.getData(id);
        if (container.requiresMerge() == false) return;

        // Only if we have the ability to write the object should we attempt to merge it
        if (d.authorization.canWrite(partition.partitionKey(), id)) {
            d.io.write(container.fetchData());
        }
    }

    public void forceMaintenanceNow()
    {
        Date milesInThePast = DateUtils.addYears(new Date(), -1);

        for (TopicAndPartition what : listPartitions()) {
            State state = states.getOrDefault(what, null);
            if (state == null) continue;

            state.tombstones.replaceAll((k, v) -> milesInThePast);
            state.merges.replaceAll((k, v) -> milesInThePast);
        }

        int curTick = ticks.get();

        // Wait until the next tick happens
        while (ticks.get() <= curTick) {
            try {
                Thread.sleep(100);
            } catch (InterruptedException e) {
                throw new RuntimeException(e);
            }
        }
    }
}
