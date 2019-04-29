package com.tokera.ate.test.dao;

import com.tokera.ate.annotations.PermitParentFree;
import com.tokera.ate.annotations.YamlTag;
import com.tokera.ate.common.ImmutalizableArrayList;
import com.tokera.ate.common.ImmutalizableTreeMap;
import com.tokera.ate.units.Alias;
import com.tokera.ate.units.DaoId;

import javax.enterprise.context.Dependent;
import javax.persistence.Column;
import javax.persistence.Table;
import java.util.UUID;

@Dependent
@YamlTag("dao.myaccount")
@Table(name = "myaccount")
@PermitParentFree
public class MyAccount extends MyBaseAccount {
    @Column
    public final ImmutalizableArrayList<@DaoId UUID> things = new ImmutalizableArrayList<>();
    @Column
    public boolean isPublic = false;
    @Column
    public final ImmutalizableTreeMap<@Alias String, @DaoId UUID> textFiles = new ImmutalizableTreeMap<>();

    public MyAccount() { }
}
