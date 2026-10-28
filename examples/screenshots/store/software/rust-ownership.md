# Ownership in Rust

Every value has exactly one **owner**. Assigning a `String` to another
variable *moves* it: the old variable can no longer be used. Types that
are `Copy`, such as integers, are copied instead.

Borrowing gives access without taking ownership: any number of shared
references (`&T`), or exactly one mutable reference (`&mut T`), never both
at once.

````srl
@ id:ma0s31
Q: Does this compile?
  ```rust
  let a = String::from("hi");
  let b = a;
  println!("{a}");
  ```
A: No. `let b = a;` moves the `String` into `b`, so `a` can't be used
  afterwards (error E0382, *borrow of moved value*).

  With `let b = a.clone();` it compiles.

@ id:91drf5
Q: At any one time, a value may have either {{c1::any number of shared references}} or {{c2::exactly one mutable reference}}.

@ id:uc2lg0
Q: Why does `let y = x;` leave `x` usable when `x` is an `i32`?
A: `i32` is `Copy`, so the value is copied instead of moved.
````
