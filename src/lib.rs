// the rules is in following order:
//  - RUSTC ALLOW
//  - RUSTC WARNING
//  - CLIPPY
// rustc rules not enabled:
//  - box_pointers
//  - missing_copy_implementations
//  - missing_debug_implementations
//  - missing_docs
//  - non_exhaustive_omitted_patterns
//  - unreachable_pub
//  - unsafe_code
//  - unused_crate_dependencies
//  - unused_qualifications
//  - unused_results
//  - variant_size_differences
#![cfg_attr(
    feature = "cargo-clippy",
    cfg_attr(feature = "c_unwind", deny(ffi_unwind_calls)),
    cfg_attr(feature = "strict_provenance", deny(fuzzy_provenance_casts, lossy_provenance_casts)),
    cfg_attr(feature = "must_not_suspend", deny(must_not_suspend)),
    cfg_attr(feature = "lint_reasons", deny(unfulfilled_lint_expectations)),
    deny(
        absolute_paths_not_starting_with_crate,
        deprecated_in_future,
        elided_lifetimes_in_paths,
        explicit_outlives_requirements,
        keyword_idents,
        let_underscore_drop,
        macro_use_extern_crate,
        meta_variable_misuse,
        missing_abi,
        non_ascii_idents,
        noop_method_call,
        pointer_structural_match,
        rust_2021_incompatible_closure_captures,
        rust_2021_incompatible_or_patterns,
        rust_2021_prefixes_incompatible_syntax,
        rust_2021_prelude_collisions,
        single_use_lifetimes,
        trivial_casts,
        trivial_numeric_casts,
        unsafe_op_in_unsafe_fn,
        unused_extern_crates,
        unused_import_braces,
        unused_lifetimes,
        unused_macro_rules,
        unused_tuple_struct_fields,
        anonymous_parameters,
        array_into_iter,
        asm_sub_register,
        bad_asm_style,
        bare_trait_objects,
        bindings_with_variant_name,
        break_with_label_and_loop,
        clashing_extern_declarations,
        coherence_leak_check,
        confusable_idents,
        const_evaluatable_unchecked,
        const_item_mutation,
        dead_code,
        deprecated_where_clause_location,
        deref_into_dyn_supertrait,
        deref_nullptr,
        drop_bounds,
        duplicate_macro_attributes,
        dyn_drop,
        ellipsis_inclusive_range_patterns,
        exported_private_dependencies,
        for_loops_over_fallibles,
        forbidden_lint_groups,
        function_item_references,
        illegal_floating_point_literal_pattern,
        improper_ctypes,
        improper_ctypes_definitions,
        incomplete_features,
        indirect_structural_match,
        inline_no_sanitize,
        invalid_doc_attributes,
        invalid_value,
        irrefutable_let_patterns,
        large_assignments,
        late_bound_lifetime_arguments,
        legacy_derive_helpers,
        mixed_script_confusables,
        named_arguments_used_positionally,
        no_mangle_generic_items,
        non_camel_case_types,
        non_fmt_panics,
        non_shorthand_field_patterns,
        non_snake_case,
        non_upper_case_globals,
        nontrivial_structural_match,
        opaque_hidden_inferred_bound,
        overlapping_range_endpoints,
        path_statements,
        private_in_public,
        redundant_semicolons,
        renamed_and_removed_lints,
        repr_transparent_external_private_fields,
        semicolon_in_expressions_from_macros,
        special_module_name,
        stable_features,
        suspicious_auto_trait_impls,
        temporary_cstring_as_ptr,
        trivial_bounds,
        type_alias_bounds,
        tyvar_behind_raw_pointer,
        uncommon_codepoints,
        unconditional_recursion,
        unexpected_cfgs,
        uninhabited_static,
        unknown_lints,
        unnameable_test_items,
        unreachable_code,
        unreachable_patterns,
        unstable_name_collisions,
        unstable_syntax_pre_expansion,
        unsupported_calling_conventions,
        unused_allocation,
        unused_assignments,
        unused_attributes,
        unused_braces,
        unused_comparisons,
        unused_doc_comments,
        unused_features,
        unused_imports,
        unused_labels,
        unused_macros,
        unused_must_use,
        unused_mut,
        unused_parens,
        unused_unsafe,
        unused_variables,
        where_clauses_object_safety,
        while_true,
        clippy::all,
        clippy::cargo,
        clippy::nursery,
        clippy::pedantic
    ),
    warn(unstable_features),
    allow(clippy::multiple_crate_versions,)
)]

mod error;
mod event;
mod exit_status;
mod handle;
mod shutdown;
mod signal;

use std::fmt;

use futures::{
    future,
    future::{Either, FutureExt},
    Future,
};
use snafu::ResultExt;
use tokio::{
    signal::unix::signal,
    sync::{mpsc, oneshot},
    task::JoinSet,
};

pub use self::{
    error::{Error, Result},
    exit_status::ExitStatus,
    handle::Handle,
    shutdown::Shutdown,
};
use crate::{event::Event, signal::UnixSignal};

pub struct LifecycleManager<ErrorType = ()> {
    handle: Handle<ErrorType>,
    event_receiver: mpsc::UnboundedReceiver<Event<ErrorType>>,
}

impl<ErrorType> Default for LifecycleManager<ErrorType>
where
    ErrorType: Send + 'static,
{
    fn default() -> Self { Self::new() }
}

impl<ErrorType> LifecycleManager<ErrorType>
where
    ErrorType: Send + 'static,
{
    #[must_use]
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let handle = Handle::new(event_sender);

        Self { handle, event_receiver }
    }

    #[inline]
    #[must_use]
    pub fn handle(&self) -> Handle<ErrorType> { self.handle.clone() }

    #[inline]
    pub fn spawn<FutureName, CreateFutureFn, Fut>(
        &self,
        name: FutureName,
        create_future: CreateFutureFn,
    ) -> Handle<ErrorType>
    where
        FutureName: fmt::Display,
        CreateFutureFn: FnOnce(Shutdown) -> Fut + Send + 'static,
        Fut: Future<Output = ExitStatus<ErrorType>> + Send + 'static,
    {
        self.handle.spawn(name, create_future)
    }

    fn init_signal_watcher(&self, sig: UnixSignal) -> Result<()> {
        tracing::debug!("Create UNIX signal listener for `{sig}`");
        let mut signal =
            signal(sig.to_signal_kind()).context(error::CreateUnixSignalListenerSnafu)?;

        let handle = self.handle();

        self.spawn(format!("UNIX signal listener ({sig})"), move |internal_signal| async move {
            tracing::debug!("Wait for signal `{sig}`");

            match future::select(internal_signal, signal.recv().boxed()).await {
                Either::Left(_) => {}
                Either::Right(_) => {
                    tracing::info!("`{sig}` received, starting graceful shutdown");
                    handle.on_signal(sig);
                }
            }

            ExitStatus::Success
        });

        Ok(())
    }

    /// # Errors
    ///
    /// - returns error while failed to join task
    pub async fn serve(mut self) -> Result<std::result::Result<(), ErrorType>> {
        let signals = [UnixSignal::Interrupt, UnixSignal::Terminate];
        for sig in signals {
            self.init_signal_watcher(sig)?;
        }

        let mut join_set = JoinSet::<()>::new();
        let mut shutdown_senders: Vec<(String, oneshot::Sender<()>)> = Vec::new();
        let mut maybe_error = None;

        while let Some(event) = self.event_receiver.recv().await {
            match event {
                Event::NewFuture { name, shutdown_sender, future } => {
                    shutdown_senders.push((name, shutdown_sender));
                    join_set.spawn(future);
                }
                Event::Signal(signal) => {
                    tracing::debug!("Receive signal `{signal}`");
                    break;
                }
                Event::Shutdown => {
                    tracing::debug!("Receive shutdown signal from internal");
                    break;
                }
                Event::FutureCompleted { name, exit_status } => {
                    match join_set.join_next().await {
                        Some(Ok(_)) => {}
                        Some(Err(err)) => {
                            tracing::error!("Error while joining tokio `Task`, error: {err}");
                        }
                        None => {
                            tracing::debug!("All futures are completed");
                            break;
                        }
                    };

                    match exit_status {
                        ExitStatus::Success => {
                            tracing::debug!("Future `{name}` completed");
                            if join_set.len() <= signals.len() {
                                break;
                            }
                        }
                        ExitStatus::Failure(error) => {
                            tracing::error!("Future `{name}` failed, starting graceful shutdown");
                            maybe_error = Some(error);
                            break;
                        }
                    }
                }
            }
        }

        for (name, sender) in shutdown_senders {
            tracing::info!("Shut down `{name}`");
            drop(sender);
        }

        while let Some(result) = join_set.join_next().await {
            result.context(error::JoinTaskHandleSnafu)?;
        }

        maybe_error.map_or_else(|| Ok(Ok(())), |err| Ok(Err(err)))
    }
}
