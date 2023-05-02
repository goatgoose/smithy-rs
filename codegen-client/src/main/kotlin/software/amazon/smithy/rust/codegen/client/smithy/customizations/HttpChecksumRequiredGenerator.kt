/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

package software.amazon.smithy.rust.codegen.client.smithy.customizations

import software.amazon.smithy.codegen.core.CodegenException
import software.amazon.smithy.model.shapes.OperationShape
import software.amazon.smithy.model.traits.HttpChecksumRequiredTrait
import software.amazon.smithy.rust.codegen.client.smithy.generators.OperationRuntimePluginCustomization
import software.amazon.smithy.rust.codegen.client.smithy.generators.OperationRuntimePluginSection
import software.amazon.smithy.rust.codegen.core.rustlang.Writable
import software.amazon.smithy.rust.codegen.core.rustlang.rust
import software.amazon.smithy.rust.codegen.core.rustlang.rustTemplate
import software.amazon.smithy.rust.codegen.core.rustlang.writable
import software.amazon.smithy.rust.codegen.core.smithy.CodegenContext
import software.amazon.smithy.rust.codegen.core.smithy.RuntimeType
import software.amazon.smithy.rust.codegen.core.smithy.customize.OperationCustomization
import software.amazon.smithy.rust.codegen.core.smithy.customize.OperationSection
import software.amazon.smithy.rust.codegen.core.smithy.generators.operationBuildError
import software.amazon.smithy.rust.codegen.core.util.hasStreamingMember
import software.amazon.smithy.rust.codegen.core.util.hasTrait
import software.amazon.smithy.rust.codegen.core.util.inputShape

class HttpChecksumRequiredGenerator(
    private val codegenContext: CodegenContext,
    private val operationShape: OperationShape,
) : OperationCustomization() {
    override fun section(section: OperationSection): Writable {
        if (!operationShape.hasTrait<HttpChecksumRequiredTrait>()) {
            return emptySection
        }
        if (operationShape.inputShape(codegenContext.model).hasStreamingMember(codegenContext.model)) {
            throw CodegenException("HttpChecksum required cannot be applied to a streaming shape")
        }
        return when (section) {
            is OperationSection.MutateRequest -> writable {
                rustTemplate(
                    """
                    ${section.request} = ${section.request}.augment(|mut req, _| {
                        let data = req
                            .body()
                            .bytes()
                            .expect("checksum can only be computed for non-streaming operations");
                        let checksum = <#{md5}::Md5 as #{md5}::Digest>::digest(data);
                        req.headers_mut().insert(
                            #{http}::header::HeaderName::from_static("content-md5"),
                            #{base64_encode}(&checksum[..]).parse().expect("checksum is valid header value")
                        );
                        Result::<_, #{BuildError}>::Ok(req)
                    })?;
                    """,
                    "md5" to RuntimeType.Md5,
                    "http" to RuntimeType.Http,
                    "base64_encode" to RuntimeType.base64Encode(codegenContext.runtimeConfig),
                    "BuildError" to codegenContext.runtimeConfig.operationBuildError(),
                )
            }

            else -> emptySection
        }
    }
}

class HttpChecksumRequiredInterceptorGenerator(
    private val codegenContext: CodegenContext,
    private val operationShape: OperationShape,
) : OperationRuntimePluginCustomization() {
    override fun section(section: OperationRuntimePluginSection): Writable {
        // Ensure that the operation has the `HttpChecksumRequiredTrait`
        if (!operationShape.hasTrait<HttpChecksumRequiredTrait>()) {
            return emptySection
        }
        // Ensure that this isn't a streaming operation, as those aren't supported by this interceptor
        if (operationShape.inputShape(codegenContext.model).hasStreamingMember(codegenContext.model)) {
            throw CodegenException("HttpChecksumRequiredInterceptor doesn't support operations with a streaming body")
        }

        return when (section) {
            is OperationRuntimePluginSection.AdditionalConfig -> writable {
                section.registerInterceptor(codegenContext.runtimeConfig, this) {
                    rust(
                        "#T::new()",
                        RuntimeType.smithyRuntime(codegenContext.runtimeConfig)
                            .resolve("http_checksum_required::HttpChecksumRequiredInterceptor"),
                    )
                }
            }

            else -> emptySection
        }
    }
}
