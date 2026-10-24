# Pkl 0.32.0 List accessor and fold prerequisite

This separate Java-only supplement observes `first`, `last`, `firstOrNull`, `lastOrNull`, and `fold` through the public string evaluation API in two fresh JVM sessions. It covers empty and nonempty lists, null elements, lazy typed object properties, fold accumulator changes, nested folds, callback shape, and error ordering. Object and accumulator results are observable values; this matrix does not claim Java reference identity. It is separate from the sealed IntSeq, math constant, and List.add fixtures.
