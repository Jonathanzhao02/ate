/*
 * To change this license header, choose License Headers in Project Properties.
 * To change this template file, choose Tools | Templates
 * and open the template in the editor.
 */
package com.tokera.ate.io.repo;

import com.google.common.cache.Cache;
import com.google.common.cache.CacheBuilder;
import com.tokera.ate.common.LoggerHook;
import com.tokera.ate.dao.GenericPartitionKey;
import com.tokera.ate.dao.MessageBundle;
import com.tokera.ate.dao.TopicAndPartition;
import com.tokera.ate.delegates.AteDelegate;
import com.tokera.ate.dto.msg.MessagePublicKeyDto;
import com.tokera.ate.events.KeysDiscoverEvent;
import com.tokera.ate.io.api.IPartitionKey;
import org.checkerframework.checker.nullness.qual.NonNull;
import org.checkerframework.checker.nullness.qual.Nullable;

import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import javax.enterprise.inject.spi.CDI;
import javax.validation.constraints.Null;

/**
 * Class used to build subscriptions to particular partitions and feed basic raw IO commands to it
 */
public class DataSubscriber {

    private AteDelegate d = AteDelegate.get();
    private final LoggerHook LOG;
    private final Mode mode;
    private final Cache<TopicAndPartition, @NonNull DataPartition> partitions;

    public enum Mode {
        Ram,
        Kafka
    }

    public DataSubscriber(Mode mode) {
        this.mode = mode;
        this.LOG = CDI.current().select(LoggerHook.class).get();
        this.partitions = CacheBuilder.newBuilder()
                .maximumSize(d.bootstrapConfig.getSubscriberMaxPartitions())
                .removalListener(p -> {
                    removePartition((DataPartition)p.getValue());
                })
                .expireAfterAccess(d.bootstrapConfig.getSubscriberPartitionTimeout(), TimeUnit.MILLISECONDS)
                .build();
    }

    private void seedTopic(DataPartition kt)
    {   
        DataPartitionChain chain = kt.getChain(false);

        KeysDiscoverEvent discovery = new KeysDiscoverEvent(kt.partitionKey());
        d.eventKeysDiscovery.fire(discovery);

        for (MessagePublicKeyDto key : discovery.getKeys()) {
            chain.addTrustKey(key);
        }
    }

    public DataPartition getPartition(TopicAndPartition partition) {
        return getPartition(partition, true);
    }
    
    public DataPartition getOrCreatePartition(IPartitionKey partition) {
        return getOrCreatePartition(partition, true);
    }
    
    public DataPartitionChain getChain(IPartitionKey partitionKey, boolean waitForLoad) {
        DataPartition partition = getOrCreatePartition(partitionKey);
        return partition.getChain(waitForLoad);
    }

    private DataPartition createPartition(IPartitionKey key) {
        IDataPartitionBridge bridge;
        if (this.mode == Mode.Ram) {
            bridge = d.ramBridgeBuilder.createPartition(key);
        } else {
            bridge = d.kafkaBridgeBuilder.createPartition(key);
        }
        DataPartition part = new DataPartition(key, bridge);

        if (this.mode == Mode.Ram) {
            GenericPartitionKey wrapKey = new GenericPartitionKey(key);
            part.feed(d.ramDataRepository.read(wrapKey), false);
        }

        seedTopic(part);
        d.debugLogging.logSubscribed(part.partitionKey());
        return part;
    }

    private void removePartition(DataPartition part) {
        IPartitionKey key = part.partitionKey();
        if (this.mode == Mode.Ram) {
            d.ramBridgeBuilder.removePartition(key);
        } else {
            d.kafkaBridgeBuilder.removePartition(key);
        }
        d.debugLogging.logUnsubscribed(part.partitionKey());
    }

    public @Nullable DataPartition getPartition(TopicAndPartition key, boolean shouldWait) {
        DataPartition ret = this.partitions.getIfPresent(key);
        if (ret != null) {
            if (shouldWait == true) {
                ret.waitTillLoaded();
            }
            return ret;
        }
        return ret;
    }

    public DataPartition getOrCreatePartition(IPartitionKey key, boolean shouldWait) {
        TopicAndPartition keyWrap = new TopicAndPartition(key);
        DataPartition ret = this.partitions.getIfPresent(keyWrap);
        if (ret != null) {
            if (shouldWait == true) {
                ret.waitTillLoaded();
            }
            return ret;
        }

        try
        {
            ret = this.partitions.get(keyWrap, () ->
                {
                    d.debugLogging.logLoadingPartition(key);
                    d.encryptor.touch(); // required as the kafka partition needs an instance reference
                    DataPartition p = createPartition(key);
                    return p;
                });
        } catch (ExecutionException ex) {
            throw new RuntimeException(ex);
        }
        
        if (shouldWait == true) {
            ret.waitTillLoaded();
        }

        return ret;
    }
    
    public void touch() {
    }

    public void destroyAll() {
        this.partitions.invalidateAll();
    }

    public void feed(TopicAndPartition where, Iterable<MessageBundle> msgs, boolean throwOnError) {
        DataPartition ret = this.partitions.getIfPresent(where);
        if (ret != null) ret.feed(msgs, throwOnError);
    }
}
