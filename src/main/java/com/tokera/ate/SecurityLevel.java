package com.tokera.ate;

import com.google.common.collect.Lists;
import com.tokera.ate.dao.enumerations.KeyType;

public class SecurityLevel {
    public Iterable<KeyType> signingTypes = Lists.newArrayList(KeyType.qtesla, KeyType.rainbow);
    public Iterable<KeyType> encryptTypes = Lists.newArrayList(KeyType.ntru, KeyType.newhope);
    public int aesStrength = 256;
    public int signingStrength = 256;
    public int encryptionStrength = 256;
    public boolean automaticKeyRotation = true;

    public SecurityLevel() {
    }

    public SecurityLevel(int aesStrength, int signingStrength, int encryptionStrength, boolean automaticKeyRotation, Iterable<KeyType> signingTypes, Iterable<KeyType> encryptTypes) {
        this.automaticKeyRotation = automaticKeyRotation;
        this.signingTypes = signingTypes;
        this.encryptTypes = encryptTypes;
        this.aesStrength = aesStrength;
        this.signingStrength = signingStrength;
        this.encryptionStrength = encryptionStrength;
    }

    public static SecurityLevel RidiculouslySecure = new SecurityLevel(256, 512, 512, true, Lists.newArrayList(KeyType.qtesla, KeyType.rainbow), Lists.newArrayList(KeyType.ntru, KeyType.newhope));
    public static SecurityLevel VeryHighlySecure = new SecurityLevel(256, 256, 256, true, Lists.newArrayList(KeyType.qtesla), Lists.newArrayList(KeyType.ntru));
    public static SecurityLevel HighlySecure = new SecurityLevel(192, 192, 192, false, Lists.newArrayList(KeyType.qtesla), Lists.newArrayList(KeyType.ntru));
    public static SecurityLevel ModeratelySecure = new SecurityLevel(128, 128, 128, false, Lists.newArrayList(KeyType.qtesla), Lists.newArrayList(KeyType.ntru));
}