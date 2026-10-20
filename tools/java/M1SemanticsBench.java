// Primitive timing only: not a Pkl evaluator or end-to-end performance claim.
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.util.Locale;

public class M1SemanticsBench {
  static final String[] OPERATIONS = {"format", "cbrt", "sin", "checked_add"};
  static volatile long sink;
  static double math(String operation, double value) {
    return switch(operation) {
      case "cbrt" -> StrictMath.cbrt(value);
      case "sin" -> StrictMath.sin(value);
      default -> throw new IllegalArgumentException(operation);
    };
  }
  static String result(String operation, long bits) {
    if (operation.equals("format")) return Double.toString(Double.longBitsToDouble(bits));
    if (operation.equals("checked_add")) {
      try { return Long.toString(Math.addExact(bits, 17)); }
      catch (ArithmeticException e) { return "integerOverflow"; }
    }
    return String.format(Locale.ROOT, "%016x", Double.doubleToLongBits(math(operation, Double.longBitsToDouble(bits))));
  }
  static long batch(String operation, long[] values, int repetitions) {
    long checksum = 0;
    for (int repeat = 0; repeat < repetitions; repeat++) {
      for (long bits : values) {
        long contribution;
        if (operation.equals("format")) {
          String value = Double.toString(Double.longBitsToDouble(bits));
          long hash = 0;
          for (int i = 0; i < value.length(); i++) hash = hash * 31 + value.charAt(i);
          contribution = hash;
        } else if (operation.equals("checked_add")) {
          try { contribution = Math.addExact(bits, 17); }
          catch (ArithmeticException e) { contribution = 0xdeadbeefL; }
        } else {
          contribution = Double.doubleToLongBits(math(operation, Double.longBitsToDouble(bits)));
        }
        checksum = Long.rotateLeft(checksum, 7) + contribution;
      }
    }
    sink = checksum;
    return checksum;
  }
  static double median(long[] values) {
    values=values.clone(); java.util.Arrays.sort(values);
    return (values[values.length/2-1]+(double)values[values.length/2])/2;
  }
  static boolean stable(java.util.ArrayList<Long> samples) {
    if(samples.size()<12) return false;
    long[] values=new long[12];
    for(int i=0;i<12;i++) values[i]=samples.get(samples.size()-12+i);
    double first=median(java.util.Arrays.copyOfRange(values,0,4));
    double last=median(java.util.Arrays.copyOfRange(values,8,12));
    double center=median(values);
    long[] deviations=new long[12];
    for(int i=0;i<12;i++) deviations[i]=(long)Math.abs(values[i]-center);
    return first>0 && center>0 && Math.abs(last-first)/first<=0.10 && median(deviations)/center<=0.10;
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 5) throw new IllegalArgumentException("mode inputs repetitions warmups measurements");
    long[] values = Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)
        .stream().mapToLong(line -> Long.parseUnsignedLong(line,16)).toArray();
    int repetitions = Integer.parseInt(args[2]), warmups = Integer.parseInt(args[3]), measurements = Integer.parseInt(args[4]);
    if(values.length == 0 || repetitions <= 0 || warmups <= 0 || measurements <= 0) throw new IllegalArgumentException("empty workload");
    for(String operation : OPERATIONS) {
      for(long bits : values) {
        System.out.printf(Locale.ROOT, "result\t%s\t%016x\t%s%n", operation, bits,
            HexFormat.of().formatHex(result(operation,bits).getBytes(StandardCharsets.UTF_8)));
      }
    }
    if(args[0].equals("check")) return;
    if(!args[0].equals("bench")) throw new IllegalArgumentException("mode");
    var windows=new java.util.ArrayList<java.util.ArrayList<Long>>();
    for(int i=0;i<OPERATIONS.length;i++) windows.add(new java.util.ArrayList<>());
    long startWarmup=System.nanoTime();
    int rounds=0;
    boolean converged;
    while(true) {
      for(int i=0;i<OPERATIONS.length;i++) {
        String operation=OPERATIONS[i];
        long start=System.nanoTime();
        long checksum=batch(operation,values,repetitions);
        long elapsed=System.nanoTime()-start;
        windows.get(i).add(elapsed);
        System.out.printf(Locale.ROOT,"sample\t%s\twarmup\t%d\t%d\t%016x%n",operation,rounds,elapsed,checksum);
      }
      rounds++;
      long elapsed=System.nanoTime()-startWarmup;
      if(rounds>=warmups && elapsed>=5_000_000_000L && windows.stream().allMatch(M1SemanticsBench::stable)) { converged=true; break; }
      if(elapsed>=20_000_000_000L) { converged=false; break; }
    }
    System.out.printf(Locale.ROOT,"warmup-status\t%s\t%d\t%d%n",converged?"stable":"inconclusive",System.nanoTime()-startWarmup,rounds);
    for(String operation:OPERATIONS) {
      for(int sample=0;sample<measurements;sample++) {
        long start=System.nanoTime();
        long checksum=batch(operation,values,repetitions);
        long elapsed=System.nanoTime()-start;
        System.out.printf(Locale.ROOT,"sample\t%s\tmeasurement\t%d\t%d\t%016x%n",operation,sample,elapsed,checksum);
      }
    }
  }
}
