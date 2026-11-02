ALTER TABLE indexed_sessions ADD COLUMN last_message TEXT;
ALTER TABLE indexed_sessions ADD COLUMN last_message_role TEXT CHECK (last_message_role IN ('user', 'agent'));
ALTER TABLE indexed_sessions ADD COLUMN message_count INTEGER CHECK (message_count >= 0 AND message_count <= 4294967295);
