# CW Snapshot Vector Map

A snapshot vector map maps keys to vectors of items, where the vectors' sets of
items can be read at any height in the past. Items can be given an expiration,
after which they automatically disappear from the vector. This minimizes
redundant storage while allowing for efficient querying of historical data on a
changing set of items.

Because this uses a `SnapshotMap` under the hood, it's important to note that
all pushes and removals occuring on a given block will be reflected on the
following block. Since expirations are computed relative to the block they are
pushed at, an expiration of 1 block means the item will never appear in the
vector. More concretely, if an item is pushed at block `n` with an expiration of
`m`, it will be included in the vector when queried at block `n + 1` up to `n +
m - 1`. The vector at block `n + m` will no longer include the item.

## Performance

All operations (push/remove/load) run in O(n). When pushing/removing, `n` refers
to the number of items in the most recent version of the vector. When loading,
`n` refers to the number of items in the vector at the given block.

Storage is optimized by only storing each pushed item once, referencing them in
snapshots by numeric IDs that are much more compact. IDs are duplicated when the
vector is changed, while items are never duplicated.

The default `load` function can paginate item loading, but it first requires
loading the entire set of IDs from storage. Thus there is some maximum number of
items that can be stored based on gas limits and storage costs. However, this
capacity is greatly increased by snapshotting IDs rather than items directly.

## Limitations

This data structure is only designed to be updated in the present and read in
the past/present. More concretely, items can only be pushed or removed at a
block greater than or equal to the last block at which an item was pushed or
removed.

Since all IDs must be loaded from storage before paginating item loading, there
is a maximum number of items that can be stored based on gas limits and storage
costs. This will vary by chain configuration but is likely quite high due to the
compact ID storage.

## Example

```rust
use cosmwasm_std::{testing::mock_dependencies, Addr};
use cw_snapshot_vector_map::{LoadedItem, SnapshotVectorMap};

let storage = &mut mock_dependencies().storage;
let svm: SnapshotVectorMap<Addr, String> = SnapshotVectorMap::new(
    "svm__items",
    "svm__next_ids",
    "svm__active",
    "svm__active__checkpoints",
    "svm__active__changelog",
);
let key = Addr::unchecked("leaf");
let first = "first".to_string();
let second = "second".to_string();

// store the first item at block 1, expiring in 10 blocks (at block 11)
svm.push(storage, &key, &first, 1, Some(10)).unwrap();

// store the second item at block 5, which does not expire
svm.push(storage, &key, &second, 5, None).unwrap();

// remove the second item (ID: 1) at height 15
svm.remove(storage, &key, 1, 15).unwrap();

// the vector at block 3 should contain only the first item
assert_eq!(
    svm.load_all(storage, &key, 3).unwrap(),
    vec![LoadedItem {
        id: 0,
        item: first.clone(),
        expiration: Some(11),
    }]
);

// the vector at block 7 should contain both items
assert_eq!(
    svm.load_all(storage, &key, 7).unwrap(),
    vec![
        LoadedItem {
            id: 0,
            item: first.clone(),
            expiration: Some(11),
        },
        LoadedItem {
            id: 1,
            item: second.clone(),
            expiration: None,
        }
    ]
);

// the vector at block 12 should contain only the first item
assert_eq!(
    svm.load_all(storage, &key, 12).unwrap(),
    vec![LoadedItem {
        id: 1,
        item: second.clone(),
        expiration: None,
    }]
);

// the vector at block 17 should contain nothing
assert_eq!(svm.load_all(storage, &key, 17).unwrap(), vec![]);
```
