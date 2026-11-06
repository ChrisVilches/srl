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
