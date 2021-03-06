# High level overview

`Host` implementation is expected to:

1. After app initialization, construct all its `System`\'s `GlobalState`s using their trait's `init` associated function;
2. Then, application loop invokes methods in the following order:
   1. Draw;
   2. Receive event batch**;
   3. Run one update round*;

\* update round means processing all messages existed before its start; messages sent during current round are processed either in course of current round, if their target hasn't been proceed yet, or during the next round, if it has been.\
\** this also produces messages for the next round.

## Halting procedure

When system component receives the corresponding message, then application shouldn't loop anymore: the current running loop should exit right before the next "receive event batch" stage. 

## Rendering

Each entity declares a set of anchors, to which its systems may attach layouts.
 