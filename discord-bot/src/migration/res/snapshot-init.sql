CREATE TABLE snapshots
(
    aggregate_type   text                                 NOT NULL,
    aggregate_id     text                                 NOT NULL,
    last_sequence    bigint CHECK (last_sequence >= 0)    NOT NULL,
    current_snapshot bigint CHECK (current_snapshot >= 0) NOT NULL,
    payload          json                                 NOT NULL,
    timestamp        timestamp with time zone DEFAULT (CURRENT_TIMESTAMP),
    PRIMARY KEY (aggregate_type, aggregate_id, last_sequence)
);