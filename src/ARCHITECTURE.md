# High level overview

`Host` implementation is expected to:

1. After app initialization, construct all its `System`\'s `GlobalState`s using their trait's `init` associated function;
2. Then, application loop invokes methods in the following order:
   1. Run one update round*;
   2. Provide DOM;
   3. Receive event batch**;

\* update round means processing all messages existed before its start; messages sent during current round are processed only during the next round.\
\** this also produces messages for the next round.

## Halting procedure

When system component receives the corresponding message, then application shouldn't loop anymore: the current running loop should exit right before the next "receive event batch" stage. 

## Rendering

All rendering start in window components. Each gets its element tree.

An element tree is produced by `Renderer::layout`, and then it's 