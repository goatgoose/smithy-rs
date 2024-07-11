/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use crate::error::UploadError;
use crate::upload::context::UploadContext;
use crate::upload::response::UploadResponseBuilder;
use crate::upload::UploadResponse;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use tokio::task;

/// Response type for a single upload object request.
#[derive(Debug)]
#[non_exhaustive]
pub struct UploadHandle {
    /// All child multipart upload tasks spawned for this upload
    pub(crate) tasks: task::JoinSet<Result<Vec<CompletedPart>, UploadError>>,
    /// The context used to drive an upload to completion
    pub(crate) ctx: UploadContext,
    /// The response that will eventually be yielded to the caller.
    response: Option<UploadResponseBuilder>,
}

impl UploadHandle {
    /// Create a new upload handle with the given request context
    pub(crate) fn new(ctx: UploadContext) -> Self {
        Self {
            tasks: task::JoinSet::new(),
            ctx,
            response: None,
        }
    }

    /// Set the initial response builder once available
    ///
    /// This is usually after `CreateMultipartUpload` is initiated (or
    /// `PutObject` is invoked for uploads less than the required MPU threshold).
    pub(crate) fn set_response(&mut self, builder: UploadResponseBuilder) {
        if builder.upload_id.is_some() {
            let upload_id = builder.upload_id.clone().expect("upload ID present");
            self.ctx.set_upload_id(upload_id);
        }

        self.response = Some(builder);
    }

    /// Consume the handle and wait for upload to complete
    pub async fn join(mut self) -> Result<UploadResponse, UploadError> {
        complete_upload(self).await
    }

    /// Abort the upload and cancel any in-progress part uploads.
    pub async fn abort(&mut self) {
        // cancel in-progress uploads
        self.tasks.abort_all();
        // join all tasks
        while let Some(_) = self.tasks.join_next().await {}

        // TODO - invoke abort multipart upload depending on the policy
        unimplemented!()
    }

    // /// Pause the upload and return a handle that can be used to resume the upload.
    // pub fn pause(mut self) -> PausedUploadHandle {
    //     unimplemented!()
    // }

    // pub fn progress() -> Progress
}

async fn complete_upload(mut handle: UploadHandle) -> Result<UploadResponse, UploadError> {
    if !handle.ctx.is_multipart_upload() {
        todo!("non mpu upload not implemented yet")
    }

    tracing::trace!(
        "completing multipart upload: upload_id={:?}",
        handle.ctx.upload_id
    );

    let mut all_parts = Vec::new();
    // join all the upload tasks
    while let Some(join_result) = handle.tasks.join_next().await {
        let result = join_result.expect("task completed");
        match result {
            Ok(mut completed_parts) => {
                all_parts.append(&mut completed_parts);
            }
            // TODO(aws-sdk-rust#1159, design) - do we want to return first error or collect all errors?
            Err(err) => {
                tracing::error!("multipart upload failed, aborting");
                handle.abort().await;
                return Err(err);
            }
        }
    }

    // complete the multipart upload
    let complete_mpu_resp = handle
        .ctx
        .client
        .complete_multipart_upload()
        .set_bucket(handle.ctx.request.bucket.clone())
        .set_key(handle.ctx.request.key.clone())
        .set_upload_id(handle.ctx.upload_id.clone())
        .multipart_upload(
            CompletedMultipartUpload::builder()
                .set_parts(Some(all_parts))
                .build(),
        )
        // TODO(aws-sdk-rust#1159) - implement checksums
        // .set_checksum_crc32()
        // .set_checksum_crc32_c()
        // .set_checksum_sha1()
        // .set_checksum_sha256()
        .set_request_payer(handle.ctx.request.request_payer.clone())
        .set_expected_bucket_owner(handle.ctx.request.expected_bucket_owner.clone())
        .set_sse_customer_algorithm(handle.ctx.request.sse_customer_algorithm.clone())
        .set_sse_customer_key(handle.ctx.request.sse_customer_key.clone())
        .set_sse_customer_key_md5(handle.ctx.request.sse_customer_key_md5.clone())
        .send()
        .await?;

    // set remaining fields from completing the multipart upload
    let resp = handle
        .response
        .take()
        .expect("response set")
        .set_e_tag(complete_mpu_resp.e_tag.clone())
        .set_expiration(complete_mpu_resp.expiration.clone())
        .set_version_id(complete_mpu_resp.version_id.clone());

    Ok(resp.build().expect("valid response"))
}
