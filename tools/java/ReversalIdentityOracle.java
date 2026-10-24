// Direct pinned Java VM identity observations for collection reversal.
import org.pkl.core.runtime.VmList;
import org.pkl.core.runtime.VmSet;

public final class ReversalIdentityOracle {
  private static int rows;

  private static void observe(String id, Object source, VmList result, VmList repeated) {
    String first = result.isEmpty() ? "" : String.valueOf(result.get(0));
    String last = result.isEmpty() ? "" : String.valueOf(result.get(result.getLength() - 1));
    System.out.println(id + "\t" + (result == source) + "\t" + (result == VmList.EMPTY)
        + "\t" + (result == repeated) + "\t" + result.getClass().getName()
        + "\t" + result.getLength() + "\t" + first + "\t" + last);
    rows++;
  }

  public static void main(String[] args) {
    if (args.length != 0) throw new IllegalArgumentException("no arguments");
    VmList emptyList = VmList.EMPTY;
    VmList oneList = VmList.of(7L);
    VmList manyList = VmList.of(1L, 2L);
    VmSet emptySet = VmSet.EMPTY;
    VmSet oneSet = VmSet.of(7L);
    VmSet manySet = VmSet.of(1L).add(2L).add(3L);
    VmSet duplicateSet = VmSet.of(1L).add(1L).add(2L);

    observe("list-empty", emptyList, emptyList.reverse(), emptyList.reverse());
    observe("list-one", oneList, oneList.reverse(), oneList.reverse());
    observe("list-many", manyList, manyList.reverse(), manyList.reverse());
    VmList twice = manyList.reverse().reverse();
    observe("list-twice", manyList, twice, manyList.reverse().reverse());
    observe("set-empty", emptySet, emptySet.reverse(), emptySet.reverse());
    observe("set-one", oneSet, oneSet.reverse(), oneSet.reverse());
    observe("set-many", manySet, manySet.reverse(), manySet.reverse());
    observe("set-duplicate", duplicateSet, duplicateSet.reverse(), duplicateSet.reverse());
    System.out.println("rows=" + rows);
  }
}
