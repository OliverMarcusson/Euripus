DELETE FROM search_documents
WHERE entity_type = 'program';

INSERT INTO search_documents (user_id, entity_type, entity_id, title, subtitle, search_text, starts_at, ends_at)
SELECT
  p.user_id,
  'program',
  p.id,
  p.title,
  p.channel_name,
  concat_ws(' ', p.title, p.channel_name, p.description),
  p.start_at,
  p.end_at
FROM programs p;
