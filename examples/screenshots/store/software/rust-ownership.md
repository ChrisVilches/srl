# Ownership in Rust

Every value has exactly one **owner**. Assigning a `String` to another
variable *moves* it: the old variable can no longer be used. Types that
are `Copy`, such as integers, are copied instead.

Borrowing gives access without taking ownership: any number of shared
references (`&T`), or exactly one mutable reference (`&mut T`), never both
at once.
