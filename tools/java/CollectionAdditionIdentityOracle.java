// Direct pinned-runtime reference identity supplement for collection concatenation.
import org.pkl.core.runtime.VmList;
import org.pkl.core.runtime.VmMap;
import org.pkl.core.runtime.VmSet;

public final class CollectionAdditionIdentityOracle {
  private static int rows;

  private static void observe(String name, Object left, Object right, Object result) {
    System.out.println(name + "\t" + (result == left) + "\t" + (result == right));
    rows++;
  }

  public static void main(String[] arguments) {
    if (arguments.length != 0) throw new IllegalArgumentException("no arguments");
    VmList list = VmList.of(1L);
    VmList otherList = VmList.of(2L);
    VmSet set = VmSet.of(1L);
    VmSet duplicateSet = VmSet.of(1L);
    VmSet otherSet = VmSet.of(2L);
    VmMap map = VmMap.EMPTY.put("a", 1L);
    VmMap overwriteMap = VmMap.EMPTY.put("a", 2L);
    VmMap otherMap = VmMap.EMPTY.put("b", 2L);

    observe("list-empty-both", VmList.EMPTY, VmList.EMPTY,
        VmList.EMPTY.concatenate(VmList.EMPTY));
    observe("list-nonempty-empty", list, VmList.EMPTY, list.concatenate(VmList.EMPTY));
    observe("list-empty-nonempty", VmList.EMPTY, otherList,
        VmList.EMPTY.concatenate(otherList));
    observe("list-nonempty-nonempty", list, otherList, list.concatenate(otherList));

    observe("set-empty-both", VmSet.EMPTY, VmSet.EMPTY,
        VmSet.EMPTY.concatenate(VmSet.EMPTY));
    observe("set-nonempty-empty", set, VmSet.EMPTY, set.concatenate(VmSet.EMPTY));
    observe("set-empty-nonempty", VmSet.EMPTY, otherSet,
        VmSet.EMPTY.concatenate(otherSet));
    observe("set-duplicate-only", set, duplicateSet, set.concatenate(duplicateSet));
    observe("set-new-element", set, otherSet, set.concatenate(otherSet));

    observe("map-empty-both", VmMap.EMPTY, VmMap.EMPTY,
        VmMap.EMPTY.concatenate(VmMap.EMPTY));
    observe("map-nonempty-empty", map, VmMap.EMPTY, map.concatenate(VmMap.EMPTY));
    observe("map-empty-nonempty", VmMap.EMPTY, otherMap,
        VmMap.EMPTY.concatenate(otherMap));
    observe("map-overwrite-only", map, overwriteMap, map.concatenate(overwriteMap));
    observe("map-new-key", map, otherMap, map.concatenate(otherMap));
    System.out.println("rows=" + rows);
  }
}
