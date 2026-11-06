// SPDX-License-Identifier: Apache-2.0
// Bounded public API oracle. The HTTP stub is not a CLI or network observation.
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpHeaders;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.Flow;
import javax.net.ssl.SSLSession;
import org.pkl.core.SecurityManagers;
import org.pkl.core.SecurityManagerException;
import org.pkl.core.http.HttpClient;
import org.pkl.core.packages.Checksums;
import org.pkl.core.packages.DependencyMetadata;
import org.pkl.core.packages.PackageResolver;
import org.pkl.core.packages.PackageUri;

public final class PackageMetadataProbe {
  private static byte[] units(String value) {
    var out = ByteBuffer.allocate(value.length() * 2);
    for (int i = 0; i < value.length(); i++) out.putChar(value.charAt(i));
    return out.array();
  }

  private static void field(Path directory, String name, String value) throws IOException {
    Files.write(directory.resolve(name + ".utf8"), value.getBytes(StandardCharsets.UTF_8),
        StandardOpenOption.CREATE_NEW);
    Files.write(directory.resolve(name + ".utf16be"), units(value), StandardOpenOption.CREATE_NEW);
  }

  private static final class FixtureClient implements HttpClient {
    private final byte[] bytes;
    private final URI expected;
    private final Path trace;
    private boolean sent;

    FixtureClient(byte[] bytes, URI expected, Path trace) {
      this.bytes = bytes;
      this.expected = expected;
      this.trace = trace;
    }

    @Override
    public <T> HttpResponse<T> send(HttpRequest request, HttpResponse.BodyHandler<T> handler,
        HttpRequestChecker checker) throws IOException, SecurityManagerException {
      if (sent || !request.uri().equals(expected) || !request.method().equals("GET")) {
        throw new AssertionError("Unexpected metadata request: " + request);
      }
      sent = true;
      checker.check(request.uri());
      Files.writeString(trace, request.method() + "\t" + request.uri() + "\n",
          StandardCharsets.UTF_8, StandardOpenOption.CREATE_NEW);
      var headers = HttpHeaders.of(Map.of(), (name, value) -> true);
      var version = java.net.http.HttpClient.Version.HTTP_1_1;
      var subscriber = handler.apply(new HttpResponse.ResponseInfo() {
        public int statusCode() { return 200; }
        public HttpHeaders headers() { return headers; }
        public java.net.http.HttpClient.Version version() { return version; }
      });
      subscriber.onSubscribe(new Flow.Subscription() {
        private boolean delivered;
        public void request(long count) {
          if (!delivered && count > 0) {
            delivered = true;
            subscriber.onNext(List.of(ByteBuffer.wrap(bytes)));
            subscriber.onComplete();
          }
        }
        public void cancel() { delivered = true; }
      });
      T body = subscriber.getBody().toCompletableFuture().join();
      return new HttpResponse<T>() {
        public int statusCode() { return 200; }
        public HttpRequest request() { return request; }
        public Optional<HttpResponse<T>> previousResponse() { return Optional.empty(); }
        public HttpHeaders headers() { return headers; }
        public T body() { return body; }
        public Optional<SSLSession> sslSession() { return Optional.empty(); }
        public URI uri() { return request.uri(); }
        public java.net.http.HttpClient.Version version() { return version; }
      };
    }

    public void close() {}
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 6) throw new IllegalArgumentException(
        "PACKAGE_URI REQUEST_URI BODY_PATH EXPECTED_OR_DASH TEST_MODE OUTPUT_DIR");
    if (!args[4].equals(System.getProperty("org.pkl.testMode", "false"))) {
      throw new IllegalArgumentException("test mode must be fixed at JVM startup");
    }
    var uri = new PackageUri(args[0]);
    var requestUri = URI.create(args[1]);
    if (!uri.getMetadataRequestUri().equals(requestUri)) {
      throw new IllegalArgumentException("request URI differs from public package URI derivation");
    }
    var directory = Path.of(args[5]);
    Files.createDirectory(directory);
    var client = new FixtureClient(Files.readAllBytes(Path.of(args[2])), requestUri,
        directory.resolveSibling("request-trace.tsv"));
    try (var resolver = PackageResolver.getInstance(SecurityManagers.defaultManager, client, null)) {
      var checksums = args[3].equals("-") ? null : new Checksums(args[3]);
      DependencyMetadata value;
      try {
        value = resolver.getDependencyMetadata(uri, checksums);
      } catch (RuntimeException error) {
        Files.writeString(directory.resolve("status.txt"), "error\n", StandardOpenOption.CREATE_NEW);
        field(directory, "error-class", error.getClass().getName());
        field(directory, "message", error.getMessage());
        return;
      }
      Files.writeString(directory.resolve("status.txt"), "ok\n", StandardOpenOption.CREATE_NEW);
      field(directory, "name", value.getName());
      field(directory, "version", value.getVersion().toString());
      field(directory, "zip-uri", value.getPackageZipUrl().toString());
      field(directory, "zip-sha256", value.getPackageZipChecksums().getSha256());
      var dependencies = directory.resolve("dependencies");
      Files.createDirectory(dependencies);
      for (var entry : value.getDependencies().entrySet()) {
        var key = entry.getKey().isEmpty() ? "empty" : HexFormat.of().formatHex(units(entry.getKey()));
        var target = dependencies.resolve(key);
        Files.createDirectory(target);
        field(target, "name", entry.getKey());
        field(target, "uri", entry.getValue().getPackageUri().toString());
        field(target, "sha256", entry.getValue().getChecksums().getSha256());
      }
    }
  }
}
