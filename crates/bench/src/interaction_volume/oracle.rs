use super::*;

pub(super) fn execute_interaction(
    conn: &mut dyn BenchConn,
    interaction: &Interaction,
    validation: &ResultValidation,
) -> Result<InteractionVerification> {
    match interaction {
        Interaction::Append {
            event_id,
            session_id,
            sequence,
            payload,
        } => {
            conn.begin_immediate()?;
            let result = (|| {
                let inserted = conn.execute(
                    "INSERT INTO interaction_events(event_id, session_id, seq, kind, payload) \
                     VALUES (?1, ?2, ?3, 'training.progress', ?4)",
                    &[
                        CellValue::Text(event_id.clone()),
                        CellValue::Integer(*session_id),
                        CellValue::Integer(*sequence),
                        CellValue::Text(payload.clone()),
                    ],
                )?;
                if inserted != 1 {
                    bail!("event append affected {inserted} rows, expected 1");
                }
                let updated = conn.execute(
                    "UPDATE interaction_sessions SET last_seq = last_seq + 1, updated_at = ?1 \
                     WHERE id = ?2",
                    &[
                        CellValue::Integer(*sequence),
                        CellValue::Integer(*session_id),
                    ],
                )?;
                if updated != 1 {
                    bail!("session progress update affected {updated} rows, expected 1");
                }
                conn.commit()
            })();
            if result.is_err() {
                let _ = conn.rollback();
            }
            result.map(|()| InteractionVerification::default())
        }
        Interaction::ReadSession { session_id } => {
            // COUNT and session state come from one statement snapshot. This is the committed
            // state oracle: a merely in-range last_seq cannot pass while concurrent appends run.
            let row = conn.query_row(
                "SELECT s.id, s.last_seq, s.state, COUNT(e.event_id) \
                 FROM interaction_sessions s \
                 LEFT JOIN interaction_events e ON e.session_id = s.id \
                 WHERE s.id = ?1 GROUP BY s.id, s.last_seq, s.state",
                &[CellValue::Integer(*session_id)],
            )?;
            let max_appends = validation.max_appends(*session_id)?;
            match row.as_slice() {
                [
                    CellValue::Integer(id),
                    CellValue::Integer(last_seq),
                    CellValue::Text(state),
                    CellValue::Integer(committed_events),
                ] if id == session_id
                    && *last_seq >= 0
                    && committed_events == last_seq
                    && (*last_seq as u64) <= max_appends
                    && state == "training" => {}
                other => {
                    bail!(
                        "session point read failed committed-state oracle for session {session_id}: {other:?} (max appends {max_appends})"
                    );
                }
            }
            Ok(InteractionVerification {
                point_reads: 1,
                ..InteractionVerification::default()
            })
        }
        Interaction::ReplayEvents { session_id } => {
            // The LEFT JOIN always returns the session sentinel. Every event row carries the
            // session's committed append count from the same statement snapshot, so zero rows
            // cannot be accepted when committed events exist.
            let rows = conn.query_all(
                "SELECT e.event_id, e.session_id, e.seq, e.kind, e.payload, s.last_seq \
                 FROM interaction_sessions s \
                 LEFT JOIN interaction_events e ON e.session_id = s.id \
                 WHERE s.id = ?1 ORDER BY e.seq DESC LIMIT 20",
                &[CellValue::Integer(*session_id)],
            )?;
            if rows.is_empty() {
                bail!("session replay returned no committed-state sentinel for {session_id}");
            }
            let committed_events = match rows[0].last() {
                Some(CellValue::Integer(value)) if *value >= 0 => *value as u64,
                other => bail!(
                    "session replay returned invalid committed count for {session_id}: {other:?}"
                ),
            };
            let max_appends = validation.max_appends(*session_id)?;
            if committed_events > max_appends {
                bail!(
                    "session replay observed {committed_events} committed events, exceeding plan maximum {max_appends}"
                );
            }
            if committed_events == 0 {
                if rows.len() != 1
                    || rows[0][..5]
                        .iter()
                        .any(|cell| !matches!(cell, CellValue::Null))
                {
                    bail!("empty session replay returned a malformed sentinel: {rows:?}");
                }
                return Ok(InteractionVerification {
                    replays: 1,
                    ..InteractionVerification::default()
                });
            }
            let expected_rows = committed_events.min(20) as usize;
            if rows.len() != expected_rows {
                bail!(
                    "session replay returned {} rows for {committed_events} committed events; expected {expected_rows}",
                    rows.len()
                );
            }
            let mut previous_sequence = None;
            for row in &rows {
                let [
                    CellValue::Text(event_id),
                    CellValue::Integer(row_session),
                    CellValue::Integer(sequence),
                    CellValue::Text(kind),
                    CellValue::Text(payload),
                    CellValue::Integer(row_committed_events),
                ] = row.as_slice()
                else {
                    bail!("session replay returned malformed row {row:?}");
                };
                let (expected_session, expected_sequence, expected_payload) =
                    validation.expected_event(event_id)?;
                if row_session != session_id
                    || row_session != expected_session
                    || *sequence < 0
                    || sequence != expected_sequence
                    || kind != "training.progress"
                    || payload != expected_payload
                    || *row_committed_events != committed_events as i64
                    || event_sequence(event_id, validation.operations_per_thread) != Some(*sequence)
                {
                    bail!(
                        "session replay row failed committed-state oracle for session {session_id}: {row:?}"
                    );
                }
                if previous_sequence.is_some_and(|previous| previous <= *sequence) {
                    bail!("session replay is not strictly descending by sequence: {rows:?}");
                }
                previous_sequence = Some(*sequence);
            }
            Ok(InteractionVerification {
                replays: 1,
                replay_rows: rows.len() as u64,
                ..InteractionVerification::default()
            })
        }
    }
}

fn event_sequence(event_id: &str, operations_per_thread: usize) -> Option<i64> {
    let (worker, operation) = event_id.strip_prefix('w')?.split_once("-o")?;
    let worker = worker.parse::<usize>().ok()?;
    let operation = operation.parse::<usize>().ok()?;
    if format!("w{worker:03}-o{operation:08}") != event_id || operation >= operations_per_thread {
        return None;
    }
    i64::try_from(
        worker
            .checked_mul(operations_per_thread)?
            .checked_add(operation)?,
    )
    .ok()
}

pub(super) fn execute_interaction_with_retry(
    conn: &mut dyn BenchConn,
    interaction: &Interaction,
    validation: &ResultValidation,
) -> Result<(u64, InteractionVerification)> {
    let started = Instant::now();
    let mut retries = 0_u64;
    for attempt in 0..MAX_INTERACTION_ATTEMPTS {
        match execute_interaction(conn, interaction, validation) {
            Ok(verification) => return Ok((retries, verification)),
            Err(error) => {
                let retryable = matches!(
                    classify_failure(&error),
                    FailureKind::Busy | FailureKind::Locked
                );
                if !retryable
                    || attempt + 1 == MAX_INTERACTION_ATTEMPTS
                    || started.elapsed() >= INTERACTION_RETRY_DEADLINE
                {
                    return Err(error);
                }
                retries = retries.saturating_add(1);
                let backoff_ms = 1_u64 << attempt.min(6);
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
        }
    }
    unreachable!("bounded retry loop always returns")
}
