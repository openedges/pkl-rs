/*
 * Copyright © 2024 Apple Inc. and the Pkl project authors. All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Java development companion of the pinned test/kotlin/.../TestResourceReader.kt.
// The unchanged test resource ServiceLoader descriptor loads this provider;
// import1b performs no reads through its test: scheme.
package org.pkl.core.resource;

import java.net.URI;
import java.util.Optional;

public final class TestResourceReader implements ResourceReader {
  @Override public boolean hasHierarchicalUris() { return false; }
  @Override public boolean isGlobbable() { return false; }
  @Override public String getUriScheme() { return "test"; }
  @Override public Optional<Object> read(URI uri) { return Optional.of("success"); }
}
