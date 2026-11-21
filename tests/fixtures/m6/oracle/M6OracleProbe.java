// SPDX-License-Identifier: Apache-2.0
// Development-only Java 0.32.0 oracle probe for Milestone M6 (Wasm & N-API differential parity).
import java.io.FileNotFoundException;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.regex.Pattern;
import org.pkl.core.Analyzer;
import org.pkl.core.DataSize;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.FileOutput;
import org.pkl.core.ImportGraph;
import org.pkl.core.Logger;
import org.pkl.core.ModuleSchema;
import org.pkl.core.ModuleSource;
import org.pkl.core.PClass;
import org.pkl.core.PClassInfo;
import org.pkl.core.PModule;
import org.pkl.core.PObject;
import org.pkl.core.Pair;
import org.pkl.core.PklBinaryDecoder;
import org.pkl.core.PklException;
import org.pkl.core.SecurityManager;
import org.pkl.core.SecurityManagerException;
import org.pkl.core.SecurityManagers;
import org.pkl.core.StackFrame;
import org.pkl.core.StackFrameTransformer;
import org.pkl.core.StackFrameTransformers;
import org.pkl.core.TypeAlias;
import org.pkl.core.ValueFormatter;
import org.pkl.core.evaluatorSettings.TraceMode;
import org.pkl.core.http.HttpClient;
import org.pkl.core.module.ModuleKey;
import org.pkl.core.module.ModuleKeyFactories;
import org.pkl.core.module.ModuleKeyFactory;
import org.pkl.core.module.PathElement;
import org.pkl.core.module.ResolvedModuleKey;
import org.pkl.core.module.ResolvedModuleKeys;
import org.pkl.core.project.Project;
import org.pkl.core.repl.ReplRequest;
import org.pkl.core.repl.ReplResponse;
import org.pkl.core.repl.ReplServer;
import org.pkl.core.resource.Resource;
import org.pkl.core.resource.ResourceReader;
import org.pkl.core.util.pklbinary.AbstractPklBinaryDecoder;
import org.pkl.thirdparty.msgpack.core.MessagePack;
import org.pkl.thirdparty.msgpack.core.MessageUnpacker;
import org.pkl.formatter.Formatter;
import org.pkl.formatter.GrammarVersion;

/**
 * Direct pinned Java 0.32.0 oracle probe for M6 Wasm & N-API differential verification.
 *
 * <p>Scenarios:
 *   eval-text <file> [--format=<fmt>] [--prop=k=v] [--env=k=v] [--color=never|always]
 *                    [--trace-mode=compact|pretty] [--root-dir=<dir>] [--module-path=<p>]
 *                    [--ca-cert=<pem>] [--http-rewrite=from=to] [--http-proxy=<uri>]
 *                    [--http-no-proxy=<host>] [--cache-dir=<dir>] [--project-dir=<dir>]
 *   eval-source-text <file> [--uri=<uri>] [--expr=<expr>] [--format=<fmt>] [--prop=k=v] [--env=k=v]
 *   eval-output-bytes <file>
 *   eval-output-files <file> <out-dir>
 *   eval-expr-pkl-binary <file> <expr>
 *   eval-decode-values <file> [--expr=<expr>]
 *   eval-schema <file>
 *   eval-custom-spi <file>
 *   eval-custom-security <file>
 *   eval-format <file> [--grammar-version=1|2]
 *   eval-repl <working-dir> <script-file>
 *   eval-analyze <base-dir> <uri...>
 *   eval-lifecycle <fast-file> <slow-file>
 *   eval-version
 */
public final class M6OracleProbe {

  private static final HexFormat HEX = HexFormat.of();

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

  private static String encodeUtf16Hex(String s) {
    StringBuilder sb = new StringBuilder(s.length() * 4);
    for (int i = 0; i < s.length(); i++) {
      sb.append(String.format("%04x", (int) s.charAt(i)));
    }
    return sb.toString();
  }

  private static String formatFloatCanonical(double d) {
    if (Double.isNaN(d)) {
      return "NaN";
    }
    if (Double.isInfinite(d)) {
      return d > 0 ? "Infinity" : "-Infinity";
    }
    if (Double.doubleToRawLongBits(d) == Double.doubleToRawLongBits(-0.0)) {
      return "-0.0";
    }
    return Double.toString(d);
  }

  /**
   * Canonical lossless decoder covering all PklBinary tags (including IntSeq, Class, TypeAlias,
   * Function, exact wire className, i64 BigInt, UTF-16 code units, owned bytes, etc.),
   * matching `tests/fixtures/m6/runners/canonical_value.mjs`.
   */
  private static final class CanonicalBinaryDecoder extends AbstractPklBinaryDecoder {
    private CanonicalBinaryDecoder(MessageUnpacker unpacker) {
      super(unpacker);
    }

    static String decodeCanonical(byte[] bytes) {
      Object val = new CanonicalBinaryDecoder(MessagePack.newDefaultUnpacker(bytes)).decode();
      return renderScalarOrPreEncoded(val);
    }

    private static String renderScalarOrPreEncoded(Object val) {
      if (val == null) return "null";
      if (val instanceof PreEncoded pe) return pe.text;
      if (val instanceof Boolean b) return "bool:" + b;
      if (val instanceof Long l) return "int:" + l;
      if (val instanceof Integer i) return "int:" + i;
      if (val instanceof Double d) return "float:" + formatFloatCanonical(d);
      if (val instanceof String s) return "str:" + encodeUtf16Hex(s);
      throw new IllegalStateException("Unexpected decoded scalar: " + val.getClass().getName());
    }

    private record PreEncoded(String text) {}

    @Override
    protected RuntimeException doFail(Exception cause, long offset, List<String> path) {
      return new RuntimeException(
          "Exception while decoding binary data at offset " + offset + ", path " + path, cause);
    }

    @Override
    protected RuntimeException doIOFail(IOException cause) {
      return new RuntimeException(cause);
    }

    @Override
    protected Object doDecodeNull() {
      return new PreEncoded("null");
    }

    @Override
    protected Object doDecodeObject(
        String className, URI moduleUri, DecodeIterator<DecodedObjectMember> iter) {
      Map<String, String> props = new TreeMap<>();
      while (iter.hasNext()) {
        DecodedObjectMember m = iter.next();
        props.put(m.key().toString(), renderScalarOrPreEncoded(m.value()));
      }
      StringBuilder sb = new StringBuilder();
      sb.append("object(").append(className).append('|').append(moduleUri).append("){");
      int idx = 0;
      for (Map.Entry<String, String> e : props.entrySet()) {
        if (idx++ > 0) sb.append(", ");
        sb.append(e.getKey()).append('=').append(e.getValue());
      }
      sb.append('}');
      return new PreEncoded(sb.toString());
    }

    @Override
    protected Object doDecodeMap(MapDecodeIterator iter) {
      StringBuilder sb = new StringBuilder("map{");
      int idx = 0;
      while (iter.hasNext()) {
        Pair<Object, Object> entry = iter.next();
        if (idx++ > 0) sb.append(", ");
        sb.append(renderScalarOrPreEncoded(entry.getFirst()))
            .append(" => ")
            .append(renderScalarOrPreEncoded(entry.getSecond()));
      }
      sb.append('}');
      return new PreEncoded(sb.toString());
    }

    @Override
    protected Object doDecodeMapping(MapDecodeIterator iter) {
      return doDecodeMap(iter);
    }

    @Override
    protected Object doDecodeList(CollectionDecodeIterator iter) {
      StringBuilder sb = new StringBuilder("list[");
      int idx = 0;
      while (iter.hasNext()) {
        if (idx++ > 0) sb.append(", ");
        sb.append(renderScalarOrPreEncoded(iter.next()));
      }
      sb.append(']');
      return new PreEncoded(sb.toString());
    }

    @Override
    protected Object doDecodeListing(CollectionDecodeIterator iter) {
      return doDecodeList(iter);
    }

    @Override
    protected Object doDecodeSet(CollectionDecodeIterator iter) {
      StringBuilder sb = new StringBuilder("set[");
      int idx = 0;
      while (iter.hasNext()) {
        if (idx++ > 0) sb.append(", ");
        sb.append(renderScalarOrPreEncoded(iter.next()));
      }
      sb.append(']');
      return new PreEncoded(sb.toString());
    }

    @Override
    protected Object doDecodeDuration(double value, org.pkl.core.DurationUnit unit) {
      return new PreEncoded("duration:" + formatFloatCanonical(value) + ":" + unit.getSymbol());
    }

    @Override
    protected Object doDecodeDataSize(double value, org.pkl.core.DataSizeUnit unit) {
      return new PreEncoded("datasize:" + formatFloatCanonical(value) + ":" + unit.getSymbol());
    }

    @Override
    protected Object doDecodePair(Object first, Object second) {
      return new PreEncoded(
          "pair(" + renderScalarOrPreEncoded(first) + ", " + renderScalarOrPreEncoded(second) + ")");
    }

    @Override
    protected Object doDecodeIntSeq(long start, long end, long step) {
      return new PreEncoded("intseq(" + start + "," + end + "," + step + ")");
    }

    @Override
    protected Object doDecodeRegex(Pattern pattern) {
      return new PreEncoded("regex:" + encodeUtf16Hex(pattern.pattern()));
    }

    @Override
    protected Object doDecodeClass(String qualifiedName, URI moduleUri) {
      return new PreEncoded("class(" + qualifiedName + "|" + moduleUri + ")");
    }

    @Override
    protected Object doDecodeTypeAlias(String qualifiedName, URI moduleUri) {
      return new PreEncoded("typealias(" + qualifiedName + "|" + moduleUri + ")");
    }

    @Override
    protected Object doDecodeFunction() {
      return new PreEncoded("function()");
    }

    @Override
    protected Object doDecodeBytes(byte[] bytes) {
      return new PreEncoded("bytes:" + HEX.formatHex(bytes));
    }
  }

  private static void runEvalText(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-text requires <file>");
    }
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String format = null;
    boolean color = false;
    boolean allowHttp = false;
    TraceMode traceMode = TraceMode.COMPACT;
    Path rootDir = null;
    Path cacheDir = null;
    Path projectDir = null;
    List<Path> modulePaths = new ArrayList<>();
    List<Path> caCerts = new ArrayList<>();
    Map<String, String> rewrites = new LinkedHashMap<>();
    String proxyAddr = null;
    List<String> noProxy = new ArrayList<>();
    Map<String, String> props = new LinkedHashMap<>();
    Map<String, String> envs = new LinkedHashMap<>();
    for (int i = 2; i < args.length; i++) {
      String a = args[i];
      if (a.startsWith("--format=")) {
        format = a.substring("--format=".length());
      } else if (a.startsWith("--prop=")) {
        String kv = a.substring("--prop=".length());
        int eq = kv.indexOf('=');
        props.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.startsWith("--env=")) {
        String kv = a.substring("--env=".length());
        int eq = kv.indexOf('=');
        envs.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.equals("--color=always")) {
        color = true;
      } else if (a.equals("--color=never")) {
        color = false;
      } else if (a.equals("--allow-http")) {
        allowHttp = true;
      } else if (a.equals("--trace-mode=pretty")) {
        traceMode = TraceMode.PRETTY;
      } else if (a.equals("--trace-mode=compact")) {
        traceMode = TraceMode.COMPACT;
      } else if (a.startsWith("--root-dir=")) {
        rootDir = Path.of(a.substring("--root-dir=".length())).toAbsolutePath().normalize();
      } else if (a.startsWith("--module-path=")) {
        modulePaths.add(Path.of(a.substring("--module-path=".length())).toAbsolutePath().normalize());
      } else if (a.startsWith("--ca-cert=")) {
        caCerts.add(Path.of(a.substring("--ca-cert=".length())).toAbsolutePath().normalize());
      } else if (a.startsWith("--http-rewrite=")) {
        String kv = a.substring("--http-rewrite=".length());
        int eq = kv.indexOf('=');
        rewrites.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.startsWith("--http-proxy=")) {
        proxyAddr = a.substring("--http-proxy=".length());
      } else if (a.startsWith("--http-no-proxy=")) {
        noProxy.add(a.substring("--http-no-proxy=".length()));
      } else if (a.startsWith("--cache-dir=")) {
        cacheDir = Path.of(a.substring("--cache-dir=".length())).toAbsolutePath().normalize();
      } else if (a.startsWith("--project-dir=")) {
        projectDir = Path.of(a.substring("--project-dir=".length())).toAbsolutePath().normalize();
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
    if (allowHttp || rootDir != null) {
      List<Pattern> allowedMods = new ArrayList<>(SecurityManagers.defaultAllowedModules);
      List<Pattern> allowedRes = new ArrayList<>(SecurityManagers.defaultAllowedResources);
      if (allowHttp) {
        allowedMods.add(Pattern.compile("^http:"));
        allowedRes.add(Pattern.compile("^http:"));
      }
      builder.setSecurityManager(
          SecurityManagers.standard(
              allowedMods,
              allowedRes,
              SecurityManagers.defaultTrustLevels,
              rootDir));
    }
    if (!modulePaths.isEmpty()) {
      org.pkl.core.module.ModulePathResolver resolver =
          new org.pkl.core.module.ModulePathResolver(modulePaths);
      List<ModuleKeyFactory> factories = new ArrayList<>();
      factories.add(ModuleKeyFactories.modulePath(resolver));
      factories.addAll(builder.getModuleKeyFactories());
      builder.setModuleKeyFactories(factories);
      builder.addResourceReader(org.pkl.core.resource.ResourceReaders.modulePath(resolver));
    }
    if (!caCerts.isEmpty() || !rewrites.isEmpty() || proxyAddr != null) {
      HttpClient.Builder hb = HttpClient.builder();
      for (Path cert : caCerts) {
        hb.addCertificates(cert);
      }
      if (!rewrites.isEmpty()) {
        Map<URI, URI> rewriteUris = new LinkedHashMap<>();
        for (Map.Entry<String, String> e : rewrites.entrySet()) {
          rewriteUris.put(URI.create(e.getKey()), URI.create(e.getValue()));
        }
        hb.setRewrites(rewriteUris);
      }
      if (proxyAddr != null) {
        hb.setProxy(URI.create(proxyAddr), noProxy);
      }
      builder.setHttpClient(hb.build());
    }
    if (cacheDir != null) {
      builder.setModuleCacheDir(cacheDir);
    }
    if (projectDir != null) {
      Project project = Project.loadFromPath(projectDir.resolve("PklProject"));
      builder.applyFromProject(project);
      if (cacheDir != null) {
        builder.setModuleCacheDir(cacheDir);
      }
    }
    try (Evaluator evaluator = builder.build()) {
      String out = evaluator.evaluateOutputText(ModuleSource.path(file));
      System.out.write(out.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalSourceText(String[] args) throws IOException {
    if (args.length < 2) {
      throw new IllegalArgumentException("eval-source-text requires <file>");
    }
    String text = Files.readString(Path.of(args[1]), StandardCharsets.UTF_8);
    String uri = null;
    String expr = null;
    String format = null;
    Map<String, String> props = new LinkedHashMap<>();
    Map<String, String> envs = new LinkedHashMap<>();
    for (int i = 2; i < args.length; i++) {
      String a = args[i];
      if (a.startsWith("--uri=")) {
        uri = a.substring("--uri=".length());
      } else if (a.startsWith("--expr=")) {
        expr = a.substring("--expr=".length());
      } else if (a.startsWith("--format=")) {
        format = a.substring("--format=".length());
      } else if (a.startsWith("--prop=")) {
        String kv = a.substring("--prop=".length());
        int eq = kv.indexOf('=');
        props.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else if (a.startsWith("--env=")) {
        String kv = a.substring("--env=".length());
        int eq = kv.indexOf('=');
        envs.put(kv.substring(0, eq), kv.substring(eq + 1));
      } else {
        throw new IllegalArgumentException("unknown arg: " + a);
      }
    }
    EvaluatorBuilder builder = baseDeterministicBuilder()
        .setExternalProperties(props)
        .setEnvironmentVariables(envs);
    if (format != null) {
      builder.setOutputFormat(format);
    }
    try (Evaluator evaluator = builder.build()) {
      ModuleSource src = uri == null ? ModuleSource.text(text) : ModuleSource.create(URI.create(uri), text);
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
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      byte[] bytes = evaluator.evaluateOutputBytes(ModuleSource.path(file));
      System.out.write(bytes);
      System.out.flush();
    }
  }

  private static void runEvalOutputFiles(String[] args) throws IOException {
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    Path outDir = Path.of(args[2]).toAbsolutePath().normalize();
    Files.createDirectories(outDir);
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      Map<String, FileOutput> files = evaluator.evaluateOutputFiles(ModuleSource.path(file));
      StringBuilder sb = new StringBuilder();
      // Preserve exact Pkl Mapping declaration order!
      for (Map.Entry<String, FileOutput> entry : files.entrySet()) {
        String rel = entry.getKey();
        Path dest = outDir.resolve(rel).normalize();
        if (!dest.startsWith(outDir)) {
          throw new IllegalArgumentException("output path escapes out-dir: " + rel);
        }
        if (dest.getParent() != null) {
          Files.createDirectories(dest.getParent());
        }
        Files.write(dest, entry.getValue().getBytes());
        sb.append(rel).append('\n');
      }
      System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalExprPklBinary(String[] args) throws IOException {
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String expr = args[2];
    String uri = null;
    for (int i = 3; i < args.length; i++) {
      if (args[i].startsWith("--uri=")) {
        uri = args[i].substring("--uri=".length());
      }
    }
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      ModuleSource src = uri == null
          ? ModuleSource.path(file)
          : ("repl:text".equals(uri)
              ? ModuleSource.text(Files.readString(file, StandardCharsets.UTF_8))
              : ModuleSource.create(URI.create(uri), Files.readString(file, StandardCharsets.UTF_8)));
      byte[] bytes = evaluator.evaluateExpressionPklBinary(src, expr);
      System.out.write(bytes);
      System.out.flush();
    }
  }

  private static void runEvalDecodeValues(String[] args) throws IOException {
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String expr = "output.value";
    String uri = null;
    for (int i = 2; i < args.length; i++) {
      if (args[i].startsWith("--expr=")) {
        expr = args[i].substring("--expr=".length());
      } else if (args[i].startsWith("--uri=")) {
        uri = args[i].substring("--uri=".length());
      } else {
        throw new IllegalArgumentException("unknown arg: " + args[i]);
      }
    }
    try (Evaluator evaluator = baseDeterministicBuilder().build()) {
      ModuleSource src = uri == null
          ? ModuleSource.path(file)
          : ("repl:text".equals(uri)
              ? ModuleSource.text(Files.readString(file, StandardCharsets.UTF_8))
              : ModuleSource.create(URI.create(uri), Files.readString(file, StandardCharsets.UTF_8)));
      byte[] bytes = evaluator.evaluateExpressionPklBinary(src, expr);
      String canonical = CanonicalBinaryDecoder.decodeCanonical(bytes);
      System.out.write((canonical + "\n").getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalSchema(String[] args) throws IOException {
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

  private static final class MemModuleKey implements ModuleKey {
    private static final Map<String, String> MODULES = Map.of(
        "/a.pkl", "greeting = \"hello from memmod\"\nvalue = trace(42)\n@Deprecated { message = \"use value\" }\nfunction oldFn() = 7\n",
        "/dir/b.pkl", "nested = true\n"
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
        return List.of(new PathElement("a.pkl", false), new PathElement("dir", true));
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
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    boolean withLogs = false;
    String uri = null;
    for (int i = 2; i < args.length; i++) {
      if (args[i].equals("--with-logs")) {
        withLogs = true;
      } else if (args[i].startsWith("--uri=")) {
        uri = args[i].substring("--uri=".length());
      }
    }
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

    ModuleKeyFactory memFactory = modUri ->
        "memmod".equals(modUri.getScheme()) ? Optional.of(new MemModuleKey(modUri)) : Optional.empty();

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
      ModuleSource src = uri == null
          ? ModuleSource.path(file)
          : ("repl:text".equals(uri)
              ? ModuleSource.text(Files.readString(file, StandardCharsets.UTF_8))
              : ModuleSource.create(URI.create(uri), Files.readString(file, StandardCharsets.UTF_8)));
      String out = evaluator.evaluateOutputText(src);
      String combined = withLogs ? ("---OUTPUT---\n" + out + "---LOGS---\n" + logs) : out;
      System.out.write(combined.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalCustomSecurity(String[] args) throws IOException {
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    String uri = null;
    for (int i = 2; i < args.length; i++) {
      if (args[i].startsWith("--uri=")) {
        uri = args[i].substring("--uri=".length());
      }
    }
    SecurityManager custom = new SecurityManager() {
      @Override
      public void checkResolveModule(URI uri) throws SecurityManagerException {
        String scheme = uri.getScheme();
        if (!"pkl".equals(scheme) && !"file".equals(scheme) && !"repl".equals(scheme)) {
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
      ModuleSource src = uri == null
          ? ModuleSource.path(file)
          : ("repl:text".equals(uri)
              ? ModuleSource.text(Files.readString(file, StandardCharsets.UTF_8))
              : ModuleSource.create(URI.create(uri), Files.readString(file, StandardCharsets.UTF_8)));
      String out = evaluator.evaluateOutputText(src);
      System.out.write(out.getBytes(StandardCharsets.UTF_8));
      System.out.flush();
    }
  }

  private static void runEvalFormat(String[] args) throws IOException {
    Path file = Path.of(args[1]).toAbsolutePath().normalize();
    GrammarVersion gv = GrammarVersion.V2;
    for (int i = 2; i < args.length; i++) {
      if (args[i].equals("--grammar-version=1")) {
        gv = GrammarVersion.V1;
      } else if (args[i].equals("--grammar-version=2")) {
        gv = GrammarVersion.V2;
      } else {
        throw new IllegalArgumentException("unknown arg: " + args[i]);
      }
    }
    String src = Files.readString(file, StandardCharsets.UTF_8);
    Formatter fmt = new Formatter();
    String pass1 = fmt.format(src, gv);
    String pass2 = fmt.format(pass1, gv);
    if (!pass1.equals(pass2)) {
      throw new IllegalStateException("Java formatter not idempotent!");
    }
    System.out.write(pass1.getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  private static void runEvalRepl(String[] args) throws IOException {
    Path workDir = Path.of(args[1]).toAbsolutePath().normalize();
    List<String> cmds = Files.readAllLines(Path.of(args[2]), StandardCharsets.UTF_8);
    StringBuilder sb = new StringBuilder();
    try (ReplServer server = new ReplServer(
        SecurityManagers.defaultManager,
        HttpClient.builder().buildLazily(),
        org.pkl.core.Loggers.noop(),
        List.of(ModuleKeyFactories.standardLibrary, ModuleKeyFactories.file),
        List.of(
            org.pkl.core.resource.ResourceReaders.environmentVariable(),
            org.pkl.core.resource.ResourceReaders.externalProperty(),
            org.pkl.core.resource.ResourceReaders.file()),
        Map.of(),
        Map.of(),
        null,
        null,
        null,
        workDir,
        StackFrameTransformers.defaultTransformer,
        false,
        TraceMode.COMPACT)) {
      int idx = 0;
      for (String raw : cmds) {
        String line = raw.trim();
        if (line.isEmpty() || line.startsWith("#")) {
          continue;
        }
        String id = "req-" + (++idx);
        List<ReplResponse> responses;
        if (line.startsWith("EVAL ")) {
          responses = server.handleRequest(new ReplRequest.Eval(id, line.substring(5), false, false));
        } else if (line.equals("RESET")) {
          responses = server.handleRequest(new ReplRequest.Reset(id));
        } else if (line.startsWith("COMPLETE ")) {
          responses = server.handleRequest(new ReplRequest.Completion(id, line.substring(9)));
        } else {
          throw new IllegalArgumentException("unknown repl cmd: " + line);
        }
        for (ReplResponse resp : responses) {
          if (resp instanceof ReplResponse.EvalSuccess ok) {
            sb.append("OK|").append(ok.getResult().replace("\n", "\\n")).append('\n');
          } else if (resp instanceof ReplResponse.EvalError err) {
            sb.append("ERR|").append(err.getMessage().replace("\n", "\\n")).append('\n');
          } else if (resp instanceof ReplResponse.Completion comp) {
            List<String> items = new ArrayList<>(comp.members);
            Collections.sort(items);
            sb.append("COMP|").append(String.join(",", items)).append('\n');
          } else {
            sb.append("OTHER|").append(resp.getClass().getSimpleName()).append('\n');
          }
        }
      }
    }
    System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  private static String normalizeUri(URI uri, Path baseDir) {
    if ("file".equals(uri.getScheme())) {
      Path p = Path.of(uri).toAbsolutePath().normalize();
      if (p.startsWith(baseDir)) {
        return "base:/" + baseDir.relativize(p).toString().replace('\\', '/');
      }
    }
    return uri.toString();
  }

  private static void runEvalAnalyze(String[] args) throws IOException {
    Path baseDir = Path.of(args[1]).toAbsolutePath().normalize();
    List<URI> uris = new ArrayList<>();
    for (int i = 2; i < args.length; i++) {
      String raw = args[i];
      if (raw.contains(":")) {
        uris.add(URI.create(raw));
      } else {
        uris.add(Path.of(raw).toAbsolutePath().normalize().toUri());
      }
    }
    Analyzer analyzer = new Analyzer(
        StackFrameTransformers.defaultTransformer,
        false,
        SecurityManagers.defaultManager,
        List.of(ModuleKeyFactories.standardLibrary, ModuleKeyFactories.file),
        null,
        null,
        HttpClient.builder().buildLazily(),
        TraceMode.COMPACT);
    ImportGraph graph = analyzer.importGraph(uris.toArray(URI[]::new));
    Map<String, Set<String>> normImports = new TreeMap<>();
    for (Map.Entry<URI, Set<ImportGraph.Import>> entry : graph.imports().entrySet()) {
      String from = normalizeUri(entry.getKey(), baseDir);
      Set<String> targets = new TreeSet<>();
      for (ImportGraph.Import imp : entry.getValue()) {
        targets.add(normalizeUri(imp.uri(), baseDir));
      }
      normImports.put(from, targets);
    }
    StringBuilder sb = new StringBuilder();
    for (Map.Entry<String, Set<String>> e : normImports.entrySet()) {
      sb.append(e.getKey()).append(" -> ").append(String.join(",", e.getValue())).append('\n');
    }
    System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  private static void runEvalLifecycle(String[] args) throws IOException {
    Path fastFile = Path.of(args[1]).toAbsolutePath().normalize();
    Path slowFile = Path.of(args[2]).toAbsolutePath().normalize();
    StringBuilder sb = new StringBuilder();

    Evaluator eval1 = baseDeterministicBuilder().build();
    String first = eval1.evaluateOutputText(ModuleSource.path(fastFile));
    String second = eval1.evaluateOutputText(ModuleSource.path(fastFile));
    sb.append("reuse_equal=").append(first.equals(second)).append('\n');
    sb.append("reuse_output=").append(first.replace("\n", "\\n")).append('\n');
    eval1.close();
    eval1.close();
    boolean closedRejected = false;
    try {
      eval1.evaluateOutputText(ModuleSource.path(fastFile));
    } catch (IllegalStateException | org.graalvm.polyglot.PolyglotException ex) {
      closedRejected = true;
    }
    sb.append("closed_rejected=").append(closedRejected).append('\n');

    boolean timeoutTriggered = false;
    try (Evaluator eval2 = baseDeterministicBuilder().setTimeout(Duration.ofSeconds(1)).build()) {
      eval2.evaluateOutputText(ModuleSource.path(slowFile));
    } catch (PklException ex) {
      String msg = ex.getMessage() == null ? "" : ex.getMessage();
      timeoutTriggered = msg.contains("timed out");
    }
    sb.append("timeout_triggered=").append(timeoutTriggered).append('\n');
    System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  private static void runEvalVersion() throws IOException {
    String version = org.pkl.core.Release.current().version().toString();
    System.out.write((version + "\n").getBytes(StandardCharsets.UTF_8));
    System.out.flush();
  }

  public static void main(String[] args) {
    if (args.length < 1) {
      System.err.println("Usage: M6OracleProbe <scenario> [args...]");
      System.exit(2);
    }
    try {
      switch (args[0]) {
        case "eval-text" -> runEvalText(args);
        case "eval-source-text" -> runEvalSourceText(args);
        case "eval-output-bytes" -> runEvalOutputBytes(args);
        case "eval-output-files" -> runEvalOutputFiles(args);
        case "eval-expr-pkl-binary" -> runEvalExprPklBinary(args);
        case "eval-decode-values" -> runEvalDecodeValues(args);
        case "eval-schema" -> runEvalSchema(args);
        case "eval-custom-spi" -> runEvalCustomSpi(args);
        case "eval-custom-security" -> runEvalCustomSecurity(args);
        case "eval-format" -> runEvalFormat(args);
        case "eval-repl" -> runEvalRepl(args);
        case "eval-analyze" -> runEvalAnalyze(args);
        case "eval-lifecycle" -> runEvalLifecycle(args);
        case "eval-version" -> runEvalVersion();
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
