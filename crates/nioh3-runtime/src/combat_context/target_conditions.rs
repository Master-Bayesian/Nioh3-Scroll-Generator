//! Read the ordinary PC202 target and the native condition inputs used by multi-weapon ranking.
use super::*;
use serde::Serialize;

#[derive(Serialize, PartialEq, Eq)]
pub struct StatusObject {
    node: u64,
    pub address: u64,
    effect: u32,
    element: u32,
    scale: u32,
    deduction: u32,
    stacks: u32,
    resource_index: i32,
    resource: Option<u32>,
}

#[derive(Serialize, PartialEq, Eq)]
pub struct TargetConditions {
    pub actor: u64,
    pub owner: u64,
    pub stat: u64,
    sentinel: u64,
    pub objects: Vec<StatusObject>,
}

/// Native 9C614C -> 85FA8 -> 9C6190. A missing resource uses no scalar.
fn linked_resource(
    m: &dyn InventoryMemory,
    object: u64,
    index: i32,
) -> Result<Option<u32>, RuntimeError> {
    if index == -1 {
        return Ok(None);
    }
    let handle = q(m, add(object, 0x48)?)?;
    if handle == 0 {
        return Ok(None);
    }
    let actor = q(m, add(handle, 0x340)?)?;
    if actor == 0 {
        return Ok(None);
    }
    let owner = q(m, add(actor, 0x3a0)?)?;
    if owner == 0 {
        return Ok(None);
    }
    let store = add(owner, 0x33f8)?;
    let subject = q(m, store)?;
    if subject == 0 || u(m, add(subject, 0xab84)?)? != u32::MAX {
        return Ok(None);
    }
    let kind = q(m, add(subject, 0xb0)?)?;
    if kind == 0 || u(m, add(kind, 0x10)?)? == 0 {
        return Ok(None);
    }
    let at = store
        .checked_add_signed(8 + 12 * i64::from(index))
        .ok_or_else(|| layout("target condition index overflow"))?;
    u(m, at).map(Some)
}

pub fn read_target_conditions(
    m: &dyn InventoryMemory,
    player: u64,
    controller: u64,
) -> Result<TargetConditions, RuntimeError> {
    let actor = q(m, add(controller, 0xb98)?)?;
    if actor == 0 || actor == player {
        return Err(layout("no distinct opponent selected"));
    }
    let owner = q(m, add(actor, 0x3a0)?)?;
    if owner == 0 || q(m, add(owner, 0x3100)?)? != actor {
        return Err(layout("opponent owner changed"));
    }
    let external = q(m, add(owner, 0x2d90)?)?;
    let stat = if external == 0 {
        add(owner, 0x25a8)?
    } else {
        external
    };
    // 2F763C starts with sentinel.next, selects object+54, and uses the
    // first object as fallback. Preserve physical list order and duplicates.
    let sentinel = q(m, add(owner, 0x33b0)?)?;
    let first = q(m, sentinel)?;
    let mut node = first;
    let mut seen = BTreeSet::new();
    let mut objects = Vec::new();
    let mut previous = sentinel;
    while node != sentinel {
        if node == 0 || !seen.insert(node) || seen.len() > 256 || q(m, add(node, 8)?)? != previous {
            return Err(layout("invalid opponent condition list"));
        }
        let object = q(m, add(node, 0x18)?)?;
        if object == 0 {
            return Err(layout("missing opponent condition object"));
        }
        let index = u(m, add(object, 0x84)?)? as i32;
        let entry = StatusObject {
            node,
            address: object,
            effect: u(m, object)?,
            element: u(m, add(object, 0x54)?)?,
            scale: u(m, add(object, 0x28)?)?,
            deduction: u(m, add(object, 0x2c)?)?,
            stacks: u(m, add(object, 0x40)?)?,
            resource_index: index,
            resource: linked_resource(m, object, index)?,
        };
        objects.push(entry);
        previous = node;
        node = q(m, node)?;
    }
    if objects.is_empty() || q(m, sentinel)? != first || q(m, add(controller, 0xb98)?)? != actor {
        return Err(layout("opponent selection or condition list changed"));
    }
    Ok(TargetConditions {
        actor,
        owner,
        stat,
        sentinel,
        objects,
    })
}

#[derive(Serialize, PartialEq, Eq)]
pub struct SourceConditions {
    pub owner_counter: u32,
    pub manager: u64,
    pub members: Vec<u32>,
}

pub fn read_source_conditions(
    m: &dyn InventoryMemory,
    owner: u64,
) -> Result<SourceConditions, RuntimeError> {
    let holder = q(m, add(m.module_base(), 79330664)?)?;
    let members = progression::graph(m, add(holder, 0x2820)?, 24, 2)?
        .into_keys()
        .collect();
    Ok(SourceConditions {
        owner_counter: u(m, add(owner, 0x3668)?)?,
        manager: holder,
        members,
    })
}
