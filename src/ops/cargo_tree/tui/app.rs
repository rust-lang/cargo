use std::{sync::Arc, sync::mpsc, thread, time::Duration};

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent};

use crate::GlobalContext;

use super::{
    TuiTree,
    dependency::DependencyTree,
    draw_tui,
    state::{
        Event, SearchRequest, SearchResult, TreeOptions, TreeResolutionRequest,
        TreeResolutionResult, TuiChannels, TuiState,
    },
    widget::{TreeCharset, TreeFormat, TreePrefix, TreeWidgetState},
};

/// Starts the interactive dependency tree.
pub fn run(
    gctx: &mut GlobalContext,
    tree_options: TreeOptions,
    prefix: TreePrefix,
    charset: TreeCharset,
    format: TreeFormat,
    inline: bool,
) -> Result<()> {
    let depth = match tree_options.resolution.display_depth {
        crate::ops::cargo_tree::DisplayDepth::MaxDisplayDepth(depth) if depth != u32::MAX => {
            Some(depth as usize)
        }
        _ => None,
    };
    let dependency_tree = Arc::new(TuiTree::new(
        DependencyTree::load_with_context_and_format(
            gctx,
            tree_options.manifest_path.clone(),
            &tree_options.resolution,
            format.requirements().with_metadata(),
        )?,
        tree_options.no_dedupe,
    ));

    let (search_tx, search_rx) = mpsc::channel::<SearchRequest>();
    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let search_event_tx = event_tx.clone();
    let worker_handle = thread::spawn(move || search_worker(search_rx, search_event_tx));
    let (resolution_tx, resolution_rx) = mpsc::channel::<TreeResolutionRequest>();

    let mut state = TuiState::new(
        dependency_tree,
        TuiChannels {
            search: search_tx,
            resolution: resolution_tx,
        },
        depth,
        prefix,
        charset,
        format,
        tree_options,
    );
    let mut terminal = Some(init_terminal(inline)?);

    while state.running {
        if let Some(terminal) = &mut terminal {
            terminal.draw(|frame| draw_tui(frame, &mut state))?;
        }

        while let Ok(event) = event_rx.try_recv() {
            let resolution_finished = matches!(&event, Event::TreeResolved(_));
            state.handle_event(event);
            if resolution_finished && terminal.is_none() {
                terminal = Some(init_terminal(inline)?);
            }
        }

        if terminal.is_none() {
            if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(16)) {
                let resolution_finished = matches!(&event, Event::TreeResolved(_));
                state.handle_event(event);
                if resolution_finished {
                    terminal = Some(init_terminal(inline)?);
                }
            }
            continue;
        }

        if event::poll(Duration::from_millis(16))?
            && let CrosstermEvent::Key(key_event) = event::read()?
        {
            state.handle_event(Event::Key(key_event));
            if state.resolution_pending() && terminal.is_some() {
                drop(terminal.take());
                ratatui::restore();
                let mut request = None;
                while let Ok(next_request) = resolution_rx.try_recv() {
                    request = Some(next_request);
                }
                if let Some(request) = request {
                    let result = DependencyTree::load_with_context_and_format(
                        gctx,
                        request.options.manifest_path.clone(),
                        &request.options.resolution,
                        request.format_requirements,
                    )
                    .map(|tree| Arc::new(TuiTree::new(tree, request.options.no_dedupe)))
                    .map_err(|error| format!("{error:#}"));
                    state.handle_event(Event::TreeResolved(TreeResolutionResult {
                        generation: request.generation,
                        options: request.options,
                        result,
                    }));
                }
                terminal = Some(init_terminal(inline)?);
            }
        }
    }

    drop(state);
    if terminal.take().is_some() {
        ratatui::restore();
    }
    let _ = worker_handle.join();
    Ok(())
}

fn init_terminal(inline: bool) -> Result<ratatui::DefaultTerminal> {
    if inline {
        let (_, height) = crossterm::terminal::size()?;
        Ok(ratatui::try_init_with_options(ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        })?)
    } else {
        Ok(ratatui::try_init()?)
    }
}

fn search_worker(search_rx: mpsc::Receiver<SearchRequest>, event_tx: mpsc::Sender<Event>) {
    while let Ok(mut request) = search_rx.recv() {
        while let Ok(next_request) = search_rx.try_recv() {
            request = next_request;
        }

        let search_state = TreeWidgetState::search(&request.dependency_tree, &request.query);
        let event = Event::SearchResult(SearchResult {
            generation: request.generation,
            query: request.query,
            search_state,
        });

        if event_tx.send(event).is_err() {
            break;
        }
    }
}
