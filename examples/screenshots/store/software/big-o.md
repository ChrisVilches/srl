# Complexity of common operations

| Structure | Lookup | Insert | Notes |
|---|---|---|---|
| Array | O(1) by index | O(n) | contiguous, cache friendly |
| Hash map | O(1) average | O(1) average | O(n) worst case on collisions |
| Balanced BST | O(log n) | O(log n) | keeps keys sorted |
| Binary heap | O(1) min | O(log n) | good for priority queues |

Amortized analysis averages the cost over a sequence of operations: a
dynamic array doubles its capacity when full, so pushes are O(1) amortized
even though one push in a while costs O(n).

```srl id:pphbmm
Q: Why is pushing onto a dynamic array O(1) *amortized* but not O(1)?
A: When it is full it doubles its capacity and copies everything, which
  costs O(n); averaged over all pushes, that is constant per push.
```

```srl id:75ryh7
Q: A hash map's lookup is O(1) on average but {{c1::O(n)}} in the worst case, when {{c2::every key collides}}.
```

```srl id:wcjimv
Q: Which structure gives the minimum in O(1) and inserts in O(log n)?
A: A binary heap.
```
