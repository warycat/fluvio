use std::io::Error;

use tracing::{trace, debug};

use fluvio_sc_schema::objects::*;
use fluvio_sc_schema::spu::SpuSpec;
use fluvio_sc_schema::objects::*;
use fluvio_controlplane_metadata::store::*;
use fluvio_future::task::spawn;
use fluvio_socket::*;

use crate::core::SharedContext;


pub struct WatchController<S> {
    response_sink: InnerExclusiveFlvSink<S>,
    context: SharedContext,
    metadata_request: WatchMetadataRequest,
    header: RequestHeader,
    end_event: Arc<Event>,
}

impl<S> ClientMetadataController<S>
where
    S: AsyncWrite + AsyncRead + Unpin + Send + ZeroCopyWrite + 'static,
{
    pub fn handle_metadata_update(
        request: RequestMessage<WatchMetadataRequest>,
        response_sink: InnerExclusiveFlvSink<S>,
        end_event: Arc<Event>,
        context: SharedContext,
    ) {
        let (header, metadata_request) = request.get_header_request();
        let controller = Self {
            response_sink,
            context,
            header,
            metadata_request,
            end_event,
        };

        controller.run();
    }


async fn dispatch(epoch: Epoch,ctx: SharedContext) {

    let read_guard = ctx.spus().store().read().await;
    let changes = read_guard.changes_since(epoch);
    drop(read_guard);

    let epoch = changes.epoch;
    let is_sync_all = changes.is_sync_all();
    let (updates, deletes) = changes.parts();
    let request = if is_sync_all {
        UpdateSpuRequest::with_all(epoch, updates.into_iter().map(|u| u.spec).collect())
    } else {
        let mut changes: Vec<SpuMsg> = updates
            .into_iter()
            .map(|v| Message::update(v.spec))
            .collect();
        let mut deletes = deletes
            .into_iter()
            .map(|d| Message::delete(d.spec))
            .collect();
        changes.append(&mut deletes);
        UpdateSpuRequest::with_changes(epoch, changes)
    };

    let mut message = RequestMessage::new_request(request);
    message.get_mut_header().set_client_id("sc");

    debug!(
        "sending to spu: {}, all: {}, changes: {}",
        spu_id,
        message.request.all.len(),
        message.request.changes.len()
    );
    sink.send_request(&message).await?;
    Ok(epoch)
}
