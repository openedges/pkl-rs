// SPDX-License-Identifier: Apache-2.0
import java.io.FileNotFoundException;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.regex.Pattern;
import org.pkl.core.Analyzer;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.FileOutput;
import org.pkl.core.ImportGraph;
import org.pkl.core.Logger;
import org.pkl.core.ModuleSchema;
import org.pkl.core.ModuleSource;
import org.pkl.core.PClass;
import org.pkl.core.PklException;
import org.pkl.core.SecurityManager;
import org.pkl.core.SecurityManagerException;
import org.pkl.core.SecurityManagers;
import org.pkl.core.StackFrame;
import org.pkl.core.StackFrameTransformer;
import org.pkl.core.StackFrameTransformers;
import org.pkl.core.evaluatorSettings.TraceMode;
import org.pkl.core.http.HttpClient;
import org.pkl.core.module.ModuleKey;
import org.pkl.core.module.ModuleKeyFactories;
import org.pkl.core.module.ModuleKeyFactory;
import org.pkl.core.module.PathElement;
import org.pkl.core.module.ResolvedModuleKey;
import org.pkl.core.module.ResolvedModuleKeys;
import org.pkl.core.project.Project;
import org.pkl.core.resource.Resource;
import org.pkl.core.resource.ResourceReader;

/**
 * Differential oracle probe for M5 Lane 1 (Public Rust Evaluator API vs Java 0.32.0 org.pkl.core).
 *
 * <p>Scenarios:
 *   eval-text <file> [--format=<fmt>] [--prop=k=v] [--env=k=v] [--color=never|always] [--trace-mode=compact|pretty]
 *   eval-source-text <text-file> [--expr=<expr>]
 *   eval-output-bytes <file>
 *   eval-output-files <file> <out-dir>
 *   eval-expr-pkl-binary <file> <expr>
 *   eval-custom-spi <file>
 *   eval-custom-security <file>
 *   eval-schema <file>
 *   project-and-analyzer <project-dir> <entry-file>
 *   lifecycle-reuse-timeout-close <fast-file> <slow-file>
 */
public final class M5ApiOracleProbe {

  private static void emitPklError(String rendered) {
    if (rendered == null) {
      rendered = "";
    }
    System.err.print(rendered);
    if (!rendered.endsWith("\n")) {
      System.err.print("\n");
    }
    System.err.flush();
  }

  private static EvaluatorBuilder baseDeterministicBuilder() {
    return EvaluatorBuilder.preconfigured()
        .setModuleCacheDir(null)
        .setExternalProperties(Map.of())
        .setEnvironmentVariables(Map.of());
  }

  private static void runEvalText(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-text requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String format = null;
    boolean color = false;
    TraceMode traceMode = TraceMode.COMPACT;
    Map<String, String> props = new LinkedHashMap<>();
    Map<String, String> envs = new LinkedHashMap<>();
    for (int i = 2; i < args.length; i++) {
      String a = args[i];
      if (a.startsWith("--format=")) {
        format = a.substring("--format=".length());
      } else if (a.startsWith("--prop=")) {
        String kv = a.substring("--prop=".length());
        int eq = kv.indexOf('=');
        if (eq < 0) {
          throw new IllegalArgumentException("invalid --prop: " + a);
        }
        props.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.startsWith("--env=")) {
        String kv = a.substring("--env=".length());
        int eq = kv.indexOf('=');
        if (eq < 0) {
          throw new IllegalArgumentException("invalid --env: " + a);
        }
        envs.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.equals("--color=always")) {
        color = true;
      } else if (a.equals("--color=never")) {
        color = false;
      } else if (a.equals("--trace-mode=pretty")) {
        traceMode = TraceMode.PRETTY;
      } else if (a.equals("--trace-mode=compact")) {
        traceMode = TraceMode.COMPACT;
      } else {
        throw new IllegalArgumentException("unknown arg: " + a);
      }
    }
    EvaluatorBuilder builder = baseDeterministicBuilder()
        .setColor(color)
        .setTraceMode(traceMode)
        .setExternalProperties(props)
        .setEnvironmentVariables(envs);
    if (format != null) {
      builder.setOutputFormat(format);
    }
    try (Evaluator evaluator = builder.build()) {
      String out = evaluator.evaluateOutputText(ModuleSource.path(file));
      System.out.write(out.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalSourceText(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-source-text requires <text-file>");
    }
    String text = Files.readString(Path.of(args[1]), StandardCharsets.UTF_8);
    String expr = null;
    for (int i = 2; i < args.length; i++) {
      if (args[i].startsWith("--expr=")) {
        expr = args[i].substring("--expr=".length());
      } else {
        throw new IllegalArgumentException("unknown arg: " + args[i]);
      }
    }
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      ModuleSource src = ModuleSource.text(text);
      if (expr == null) {
        String out = evaluator.evaluateOutputText(src);
        System.out.write(out.getBytes(StandardCharsets.UTF_8));
      } else {
        String out = evaluator.evaluateExpressionString(src, expr);
        System.out.write((out + "\n").getBytes(StandardCharsets.UTF_8));
      }
      System.out.flush();
    }
  }

  private static void runEvalOutputBytes(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-output-bytes requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      byte[] bytes = evaluator.evaluateOutputBytes(ModuleSource.path(file));
      System.out.write(bytes);
      System.out.flush();
    }
  }

  private static void runEvalOutputFiles(String[] args) throws IOException {
    if (args.length < 3) {
      throw new IllegalArgumentException("eval-output-files requires <file> <out-dir>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    Path outDir = Path.of(args[2]).toAbsolutePath().normalize();
    Files.createDirectories(outDir);
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      Map<String, FileOutput> files = evaluator.evaluateOutputFiles(ModuleSource.path(file));
      List<String> keys = new ArrayList<>(files.keySet());
      Collections.sort(keys);
      StringBuilder sb = new StringBuilder();
      for (String rel : keys) {
        Path dest = outDir.resolve(rel).normalize();
        if (!dest.startsWith(outDir)) {
          throw new IllegalArgumentException("output path escapes out-dir: " + rel);
        }
        if (dest.getParent() != null) {
          Files.createDirectories(dest.getParent());
        }
        Files.write(dest, files.get(rel).getBytes());
        sb.append(rel).append('\n');
      }
      System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalExprPklBinary(String[] args) throws IOException {
    if (args.length < 3) {
      throw new IllegalArgumentException("eval-expr-pkl-binary requires <file> <expr>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String expr = args[2];
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      byte[] bytes = evaluator.evaluateExpressionPklBinary(ModuleSource.path(file), expr);
      System.out.write(bytes);
      System.out.flush();
    }
  }

  private static final class MemModuleKey implements ModuleKey {
    private static final Map<String, String> MODULES = Map.of(
        "/a.pkl", "greeting = \"hello from memmod\"\nvalue = trace(42)\n@Deprecated { message = \"use value\" }\nfunction oldFn() = 7\n",
        "/dir/b.pkl", "nested = true\n",
        "/boom.pkl", "fail = throw(\"memmod-boom\")\n"
    );

    private final URI uri;

    MemModuleKey(URI uri) {
      this.uri = uri;
    }

    @Override
    public URI getUri() {
      return uri;
    }

    @Override
    public boolean hasHierarchicalUris() {
      return true;
    }

    @Override
    public boolean isGlobbable() {
      return true;
    }

    @Override
    public boolean isLocal() {
      return false;
    }

    @Override
    public ResolvedModuleKey resolve(SecurityManager securityManager)
        throws IOException, SecurityManagerException {
      securityManager.checkResolveModule(uri);
      String content = MODULES.get(uri.getPath());
      if (content == null) {
        throw new FileNotFoundException("memmod not found: " + uri);
      }
      return ResolvedModuleKeys.virtual(this, uri, content, false);
    }

    @Override
    public boolean hasElement(SecurityManager securityManager, URI elementUri)
        throws SecurityManagerException {
      securityManager.checkResolveModule(elementUri);
      String p = elementUri.getPath();
      return MODULES.containsKey(p) || "/".equals(p) || "/dir".equals(p);
    }

    @Override
    public List<PathElement> listElements(SecurityManager securityManager, URI baseUri)
        throws SecurityManagerException {
      securityManager.checkResolveModule(baseUri);
      String p = baseUri.getPath();
      if ("/".equals(p) || p == null || p.isEmpty()) {
        return List.of(
            new PathElement("a.pkl", false),
            new PathElement("boom.pkl", false),
            new PathElement("dir", true));
      }
      if ("/dir".equals(p) || "/dir/".equals(p)) {
        return List.of(new PathElement("b.pkl", false));
      }
      return List.of();
    }
  }

  private static final class MemResourceReader implements ResourceReader {
    private static final Map<String, String> RESOURCES = Map.of(
        "/secret.txt", "top-secret-42",
        "/items/one.txt", "1",
        "/items/two.txt", "2"
    );

    @Override
    public String getUriScheme() {
      return "memres";
    }

    @Override
    public boolean hasHierarchicalUris() {
      return true;
    }

    @Override
    public boolean isGlobbable() {
      return true;
    }

    @Override
    public Optional<Object> read(URI uri) {
      String content = RESOURCES.get(uri.getPath());
      if (content == null) {
        return Optional.empty();
      }
      return Optional.of(new Resource(uri, content.getBytes(StandardCharsets.UTF_8)));
    }

    @Override
    public boolean hasElement(SecurityManager securityManager, URI elementUri)
        throws SecurityManagerException {
      securityManager.checkResolveResource(elementUri);
      String p = elementUri.getPath();
      return RESOURCES.containsKey(p) || "/".equals(p) || "/items".equals(p);
    }

    @Override
    public List<PathElement> listElements(SecurityManager securityManager, URI baseUri)
        throws SecurityManagerException {
      securityManager.checkResolveResource(baseUri);
      String p = baseUri.getPath();
      if ("/items".equals(p) || "/items/".equals(p)) {
        return List.of(new PathElement("one.txt", false), new PathElement("two.txt", false));
      }
      if ("/".equals(p) || p == null || p.isEmpty()) {
        return List.of(new PathElement("items", true), new PathElement("secret.txt", false));
      }
      return List.of();
    }
  }

  private static void runEvalCustomSpi(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-custom-spi requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    List<Pattern> allowedModules = new ArrayList<>(SecurityManagers.defaultAllowedModules);
    allowedModules.add(Pattern.compile("^memmod:"));
    List<Pattern> allowedResources = new ArrayList<>(SecurityManagers.defaultAllowedResources);
    allowedResources.add(Pattern.compile("^memres:"));

    StringBuilder logs = new StringBuilder();
    Logger logger = new Logger() {
      @Override
      public void trace(String message, StackFrame frame) {
        logs.append("TRACE|")
            .append(frame.getModuleUri())
            .append('|')
            .append(frame.getStartLine())
            .append('|')
            .append(message)
            .append('\n');
      }

      @Override
      public void warn(String message, StackFrame frame) {
        logs.append("WARN|")
            .append(frame.getModuleUri())
            .append('|')
            .append(frame.getStartLine())
            .append('|')
            .append(message)
            .append('\n');
      }
    };

    StackFrameTransformer transformer =
        StackFrameTransformers.defaultTransformer.andThen(
            frame -> frame.getModuleUri().startsWith("memmod:")
                ? frame.withModuleUri(frame.getModuleUri() + "?transformed=1")
                : frame);

    ModuleKeyFactory memFactory = uri ->
        "memmod".equals(uri.getScheme()) ? Optional.of(new MemModuleKey(uri)) : Optional.empty();

    EvaluatorBuilder builder = baseDeterministicBuilder()
        .setAllowedModules(allowedModules)
        .setAllowedResources(allowedResources)
        .setLogger(logger)
        .setStackFrameTransformer(transformer)
        .addResourceReader(new MemResourceReader());
    List<ModuleKeyFactory> factories = new ArrayList<>();
    factories.add(memFactory);
    factories.addAll(builder.getModuleKeyFactories());
    builder.setModuleKeyFactories(factories);

    try (Evaluator evaluator = builder.build()) {
      String out = evaluator.evaluateOutputText(ModuleSource.path(file));
      String combined = "---OUTPUT---\n" + out + "---LOGS---\n" + logs;
      System.out.write(combined.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalCustomSecurity(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-custom-security requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    SecurityManager custom = new SecurityManager() {
      @Override
      public void checkResolveModule(URI uri) throws SecurityManagerException {
        if (!"pkl".equals(uri.getScheme()) && !"file".equals(uri.getScheme())) {
          throw new SecurityManagerException("Custom security denied module: " + uri);
        }
      }

      @Override
      public void checkImportModule(URI importingModule, URI importedModule)
          throws SecurityManagerException {
        checkResolveModule(importedModule);
      }

      @Override
      public void checkReadResource(URI resource) throws SecurityManagerException {
        if ("prop".equals(resource.getScheme()) && "pkl.outputFormat".equals(resource.getSchemeSpecificPart())) {
          return;
        }
        throw new SecurityManagerException("Custom security denied resource: " + resource);
      }

      @Override
      public void checkResolveResource(URI resource) throws SecurityManagerException {
        checkReadResource(resource);
      }
    };

    EvaluatorBuilder builder = EvaluatorBuilder.unconfigured()
        .setStackFrameTransformer(StackFrameTransformers.defaultTransformer)
        .setSecurityManager(custom)
        .setModuleCacheDir(null)
        .addModuleKeyFactory(ModuleKeyFactories.standardLibrary)
        .addModuleKeyFactory(ModuleKeyFactories.file)
        .addResourceReader(org.pkl.core.resource.ResourceReaders.externalProperty())
        .addResourceReader(org.pkl.core.resource.ResourceReaders.environmentVariable());

    try (Evaluator evaluator = builder.build()) {
      String out = evaluator.evaluateOutputText(ModuleSource.path(file));
      System.out.write(out.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalSchema(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-schema requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      ModuleSchema schema = evaluator.evaluateSchema(ModuleSource.path(file));
      StringBuilder sb = new StringBuilder();
      sb.append("module=").append(schema.getModuleName()).append('\n');
      sb.append("isAmend=").append(schema.isAmend()).append('\n');
      sb.append("isExtend=").append(schema.isExtend()).append('\n');
      sb.append("supermodule=")
          .append(schema.getSupermodule() == null ? "none" : schema.getSupermodule().getModuleName())
          .append('\n');
      sb.append("docComment=")
          .append(schema.getDocComment() == null ? "none" : schema.getDocComment().replace("\n", "\\n"))
          .append('\n');

      List<String> modProps = new ArrayList<>(schema.getModuleClass().getProperties().keySet());
      Collections.sort(modProps);
      sb.append("properties=").append(String.join(",", modProps)).append('\n');

      List<String> modMethods = new ArrayList<>(schema.getModuleClass().getMethods().keySet());
      Collections.sort(modMethods);
      sb.append("methods=").append(String.join(",", modMethods)).append('\n');

      List<String> classNames = new ArrayList<>(schema.getClasses().keySet());
      Collections.sort(classNames);
      for (String cname : classNames) {
        PClass cls = schema.getClasses().get(cname);
        List<String> cprops = new ArrayList<>(cls.getProperties().keySet());
        Collections.sort(cprops);
        List<String> cmeths = new ArrayList<>(cls.getMethods().keySet());
        Collections.sort(cmeths);
        sb.append("class=")
            .append(cname)
            .append("|abstract=").append(cls.isAbstract())
            .append("|open=").append(cls.isOpen())
            .append("|props=").append(String.join(",", cprops))
            .append("|methods=").append(String.join(",", cmeths))
            .append('\n');
      }

      List<String> aliases = new ArrayList<>(schema.getTypeAliases().keySet());
      Collections.sort(aliases);
      sb.append("typeAliases=").append(String.join(",", aliases)).append('\n');

      List<String> imports = new ArrayList<>(schema.getImports().keySet());
      Collections.sort(imports);
      sb.append("imports=").append(String.join(",", imports)).append('\n');

      System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static String normalizeGraphUri(URI uri, Path projectDir) {
    if ("file".equals(uri.getScheme())) {
      Path p = Path.of(uri).toAbsolutePath().normalize();
      if (p.startsWith(projectDir)) {
        return "project:/" + projectDir.relativize(p).toString().replace('\\', '/');
      }
    }
    return uri.toString();
  }

  private static void runProjectAndAnalyzer(String[] args) throws IOException {
    if (args.length < 3) {
      throw new IllegalArgumentException("project-and-analyzer requires <project-dir> <entry-file>");
    }
    Path projectDir = Path.of(args[1]).toAbsolutePath().normalize();
    Path entryFile = Path.of(args[2]).toAbsolutePath().normalize();
    Project project = Project.loadFromPath(projectDir.resolve("PklProject"));

    String evalOut;
    EvaluatorBuilder builder = baseDeterministicBuilder()
        .applyFromProject(project)
        .setModuleCacheDir(null);
    try (Evaluator evaluator = builder.build()) {
      evalOut = evaluator.evaluateOutputText(ModuleSource.path(entryFile));
    }

    Analyzer analyzer = new Analyzer(
        StackFrameTransformers.defaultTransformer,
        false,
        SecurityManagers.defaultManager,
        List.of(ModuleKeyFactories.standardLibrary, ModuleKeyFactories.file),
        null,
        project.getDependencies(),
        HttpClient.builder().buildLazily(),
        TraceMode.COMPACT);
    ImportGraph graph = analyzer.importGraph(entryFile.toUri());

    Map<String, Set<String>> normImports = new TreeMap<>();
    for (Map.Entry<URI, Set<ImportGraph.Import>> entry : graph.imports().entrySet()) {
      String from = normalizeGraphUri(entry.getKey(), projectDir);
      Set<String> targets = new TreeSet<>();
      for (ImportGraph.Import imp : entry.getValue()) {
        targets.add(normalizeGraphUri(imp.uri(), projectDir));
      }
      normImports.put(from, targets);
    }

    StringBuilder sb = new StringBuilder();
    sb.append("---PROJECT-EVAL---\n");
    sb.append(evalOut);
    sb.append("---IMPORT-GRAPH---\n");
    for (Map.Entry<String, Set<String>> e : normImports.entrySet()) {
      sb.append(e.getKey()).append(" -> ").append(String.join(",", e.getValue())).append('\n');
    }
    System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  private static void runLifecycle(String[] args) throws IOException {
    if (args.length < 3) {
      throw new IllegalArgumentException("lifecycle-reuse-timeout-close requires <fast-file> <slow-file>");
    }
    Path fastFile = Path.of(args[1]).toAbsolutePath().normalize();
    Path slowFile = Path.of(args[2]).toAbsolutePath().normalize();
    StringBuilder sb = new StringBuilder();

    // 1. Module reuse & idempotent close + post-close rejection
    Evaluator eval1 = baseDeterministicBuilder().build();
    String first = eval1.evaluateOutputText(ModuleSource.path(fastFile));
    String second = eval1.evaluateOutputText(ModuleSource.path(fastFile));
    sb.append("reuse_equal=").append(first.equals(second)).append('\n');
    sb.append("reuse_output=").append(first.replace("\n", "\\n")).append('\n');
    eval1.close();
    eval1.close(); // idempotent close
    boolean closedRejected = false;
    try {
      eval1.evaluateOutputText(ModuleSource.path(fastFile));
    } catch (IllegalStateException | org.graalvm.polyglot.PolyglotException ex) {
      closedRejected = true;
    }
    sb.append("closed_rejected=").append(closedRejected).append('\n');

    // 2. Timeout enforcement (1s allows cold JVM polyglot enter while deterministically interrupting slow loop)
    boolean timeoutTriggered = false;
    String timeoutHeader = "";
    try (Evaluator eval2 = baseDeterministicBuilder().setTimeout(Duration.ofSeconds(1)).build()) {
      eval2.evaluateOutputText(ModuleSource.path(slowFile));
    } catch (PklException ex) {
      String msg = ex.getMessage() == null ? "" : ex.getMessage();
      timeoutTriggered = msg.contains("timed out");
      int nl = msg.indexOf('\n');
      int nl2 = nl >= 0 ? msg.indexOf('\n', nl + 1) : -1;
      timeoutHeader = nl2 >= 0 ? msg.substring(0, nl2) : msg;
    }
    sb.append("timeout_triggered=").append(timeoutTriggered).append('\n');
    sb.append("timeout_header=").append(timeoutHeader.replace("\n", "\\n")).append('\n');

    System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  public static void main(String[] args) {
    if (args.length < 1) {
      System.err.println("Usage: M5ApiOracleProbe <scenario> [args...]");
      System.exit(2);
    }
    try {
      switch (args[0]) {
        case "eval-text" -> runEvalText(args);
        case "eval-source-text" -> runEvalSourceText(args);
        case "eval-output-bytes" -> runEvalOutputBytes(args);
        case "eval-output-files" -> runEvalOutputFiles(args);
        case "eval-expr-pkl-binary" -> runEvalExprPklBinary(args);
        case "eval-custom-spi" -> runEvalCustomSpi(args);
        case "eval-custom-security" -> runEvalCustomSecurity(args);
        case "eval-schema" -> runEvalSchema(args);
        case "project-and-analyzer" -> runProjectAndAnalyzer(args);
        case "lifecycle-reuse-timeout-close" -> runLifecycle(args);
        default -> {
          System.err.println("Unknown scenario: " + args[0]);
          System.exit(2);
        }
      }
    } catch (PklException ex) {
      emitPklError(ex.getMessage());
      System.exit(1);
    } catch (Throwable ex) {
      System.err.println(ex.getClass().getName() + ": " + ex.getMessage());
      System.exit(2);
    }
  }
}
