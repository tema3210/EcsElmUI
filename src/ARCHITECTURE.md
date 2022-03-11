# High level overview

`Host` implementation is expected to:

1. After app initialization, construct all its `System`\'s `GlobalState`s using their trait's `init` associated function;
2. Then, application loop invokes methods in the following order:
   1. Run one update round*;
   2. Provide DOM;
   3. Receive event batch;

\* update round means processing all messages existed before its start; messages send during the round are processed only during the next round.

## Halting procedure

When system component recieves corresponding message, then application shouldn't loop anymore: the current running loop should exit right before the next "recieve event batch" stage. 
