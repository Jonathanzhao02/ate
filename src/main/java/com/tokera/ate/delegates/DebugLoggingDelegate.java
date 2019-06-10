package com.tokera.ate.delegates;

import com.tokera.ate.common.LoggerHook;
import com.tokera.ate.dao.base.BaseDao;
import com.tokera.ate.dao.msg.MessageBase;
import com.tokera.ate.dto.msg.*;
import com.tokera.ate.io.api.IPartitionKey;
import com.tokera.ate.io.repo.DataPartitionChain;
import com.tokera.ate.io.repo.DataStagingManager;
import com.tokera.ate.scopes.Startup;
import org.apache.commons.lang.exception.ExceptionUtils;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.checkerframework.checker.nullness.qual.Nullable;

import javax.enterprise.context.ApplicationScoped;
import java.util.UUID;
import java.util.logging.Logger;

/**
 * Delegate used to perform some extra logging for debug purposes
 */
@Startup
@ApplicationScoped
public class DebugLoggingDelegate {
    AteDelegate d = AteDelegate.get();

    public void logMergeDeferred(DataStagingManager staging, @Nullable LoggerHook LOG) {
        if (d.bootstrapConfig.isLoggingWrites()) {
            StringBuilder sb = new StringBuilder();
            sb.append("merge_deferred: [cnt=");
            sb.append(staging.size());
            sb.append("]");

            if (d.bootstrapConfig.isLoggingWithStackTrace()) {
                String fullStackTrace = ExceptionUtils.getFullStackTrace(new Throwable());
                sb.append("\n");
                sb.append(fullStackTrace);
            }
            logInfo(sb.toString(), LOG);
        }
    }

    public void logDelete(IPartitionKey part, MessageDataDto data, @Nullable LoggerHook LOG) {
        if (d.bootstrapConfig.isLoggingDeletes()) {
            StringBuilder sb = new StringBuilder();
            sb.append("remove: [->");
            sb.append(part);
            sb.append(":");
            sb.append(data.getHeader().getId());
            sb.append("]");
            if (d.bootstrapConfig.isLoggingMessages()) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(data));
            }
            logInfo(sb.toString(), LOG);
        }
    }

    public void logDelete(BaseDao entity, @Nullable LoggerHook LOG) {
        if (d.bootstrapConfig.isLoggingDeletes()) {
            StringBuilder sb = new StringBuilder();
            sb.append("remove: [->");
            sb.append(entity.addressableId());
            sb.append("]");
            if (d.bootstrapConfig.isLoggingData()) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(entity));
            }
            logInfo(sb.toString(), LOG);
        }
    }

    public void logMerge(@Nullable MessageDataDto data, @Nullable BaseDao entity, @Nullable LoggerHook LOG, boolean later)
    {
        if (d.bootstrapConfig.isLoggingWrites()) {
            MessageDataHeaderDto header = data != null ? data.getHeader() : null;

            StringBuilder sb = new StringBuilder();

            if (later) {
                sb.append("write_later:");
            } else {
                sb.append("write_now:");
            }

            UUID id = header != null ? header.getId() : (entity != null ? entity.getId() : null);
            if (id != null) {
                sb.append(" [->");
                sb.append(id);
                sb.append("]");
            }

            String payloadClazz = header != null ? header.getPayloadClazz() : (entity != null ? entity.getClass().getName() : null);
            if (payloadClazz != null) {
                sb.append(" ");
                sb.append(payloadClazz);
            }

            UUID parentId = header != null ? header.getParentId() : (entity != null ? entity.getParentId() : null);
            if (parentId != null) {
                sb.append(" parent=");
                sb.append(parentId);
            }
            if (d.bootstrapConfig.isLoggingMessages() && data != null) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(data));
            }
            if (d.bootstrapConfig.isLoggingData() && entity != null) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(entity));
            }
            logInfo(sb.toString(), LOG);
        }
    }

    public void logTrust(IPartitionKey part, MessagePublicKeyDto trustedKey, @Nullable LoggerHook LOG) {
        if (d.bootstrapConfig.isLoggingChainOfTrust()) {
            StringBuilder sb = new StringBuilder();
            sb.append("trust: [->");
            sb.append(part);
            sb.append(":");
            sb.append(trustedKey.getPublicKeyHash());
            sb.append("] ");

            if (trustedKey instanceof MessagePrivateKeyDto) {
                sb.append("privateKey");
            } else {
                sb.append("publicKey");
            }

            if (d.bootstrapConfig.isLoggingMessages()) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(trustedKey));
            }

            logInfo(sb.toString(), LOG);
        }
    }

    public void logTrust(IPartitionKey part, MessageDataDto data, @Nullable LoggerHook LOG)
    {
        if (d.bootstrapConfig.isLoggingChainOfTrust()) {
            logTrust(part, data.getHeader(), LOG);
        }
    }

    public void logTrust(IPartitionKey part, MessageDataHeaderDto header, @Nullable LoggerHook LOG)
    {
        if (d.bootstrapConfig.isLoggingChainOfTrust()) {
            StringBuilder sb = new StringBuilder();
            sb.append("trust: [->");
            sb.append(part);
            sb.append("] data_commit: ");
            sb.append(header.getPayloadClazz());
            sb.append(":");
            sb.append(header.getId());

            sb.append(" attached to ");
            sb.append(header.getParentId());

            if (d.bootstrapConfig.isLoggingMessages()) {
                sb.append("\n");
                sb.append(d.yaml.serializeObj(header));
            }

            logInfo(sb.toString(), LOG);
        }
    }

    public void logReceive(MessageBaseDto msg, @Nullable LoggerHook LOG)
    {
        if (d.bootstrapConfig.isLoggingMessages()) {
            new LoggerHook(DataPartitionChain.class).info("rcv:\n" + d.yaml.serializeObj(msg));
        }
    }

    public void logKafkaRecord(ConsumerRecord<String, MessageBase> record, @Nullable LoggerHook LOG) {
        if (d.bootstrapConfig.isLoggingKafka()) {
            StringBuilder sb = new StringBuilder();

            sb.append("record(topic=");
            sb.append(record.topic());
            sb.append(", partition=");
            sb.append(record.partition());
            sb.append(", id=");
            sb.append(record.key());
            sb.append(")");

            logInfo(sb.toString(), LOG);
        }
    }

    public void logInfo(String info, @Nullable LoggerHook LOG) {
        if (LOG != null) {
            LOG.info(info);
        } else {
            d.genericLogger.info(info);
        }
    }
}