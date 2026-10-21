// Oracle-only observation of a real resource callback during annotation construction.
import java.net.URI;
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.regex.Pattern;
import org.pkl.core.*;
import org.pkl.core.resource.ResourceReader;

public final class M2ClassLifecycleHostProbe {
  static final class Reader implements ResourceReader {
    int calls;
    Evaluator evaluator;
    ModuleSource source;
    public String getUriScheme() { return "life"; }
    public boolean hasHierarchicalUris() { return false; }
    public boolean isGlobbable() { return false; }
    public Optional<Object> read(URI uri) {
      calls++;
      System.out.println("host-read=" + uri + " calls=" + calls);
      System.out.println("inner-class-identity=" + evaluator.evaluateExpressionString(source, "Child == Child"));
      System.out.println("inner-partial-assignment=" + evaluator.evaluateExpressionString(source, "new Child { fresh = 77 }.fresh"));
      return Optional.of("resource");
    }
  }
  public static void main(String[] args) throws Exception {
    Path out = Path.of(args[0]);
    Reader reader = new Reader();
    try (var evaluator = EvaluatorBuilder.preconfigured().setColor(false).setModuleCacheDir(null)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
        .setAllowedResources(List.of(Pattern.compile("life:.*"))).setResourceReaders(List.of(reader)).build()) {
      var source = ModuleSource.create(URI.create("file:///m2-platform.pkl"), Files.readString(out.resolve("input.pkl")));
      reader.evaluator = evaluator;
      reader.source = source;
      for (int index = 0; index < 2; index++) {
        try {
          var result = evaluator.evaluateExpressionString(source, "trigger");
          Files.writeString(out.resolve(index + ".stdout.bin"), result);
          Files.writeString(out.resolve(index + ".stderr.bin"), "");
        } catch (PklException error) {
          Files.writeString(out.resolve(index + ".stdout.bin"), "");
          Files.writeString(out.resolve(index + ".stderr.bin"), error.getMessage(), StandardCharsets.UTF_8);
        }
        System.out.println("attempt=" + index + " calls=" + reader.calls);
      }
    }
  }
}
