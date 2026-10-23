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
// Java development companion of the pinned test/kotlin/.../TestModuleKeyFactory.kt.
// Keeps the original test resource ServiceLoader descriptor usable without a
// Kotlin compiler. This provider is not selected by the import1b input.
package org.pkl.core.module;

import java.net.URI;
import java.util.Optional;

public final class TestModuleKeyFactory implements ModuleKeyFactory {
  @Override
  public Optional<ModuleKey> create(URI uri)
      throws java.net.URISyntaxException, java.io.IOException,
          org.pkl.core.externalreader.ExternalReaderProcessException {
    return uri.getScheme().equals("test")
        ? ModuleKeyFactories.classPath(getClass().getClassLoader())
            .create(URI.create("modulepath:/org/pkl/core/module/testFactoryTest.pkl"))
        : Optional.empty();
  }
}
