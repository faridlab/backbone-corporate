-- Incoterms 2020 — the eleven ICC delivery terms.
--
-- Incoterms are an international standard published by the International
-- Chamber of Commerce, in force since 1 January 2020, and they are the same
-- eleven terms everywhere. Nothing here is Indonesia-specific.
--
-- Two families: seven that apply to any mode of transport, and four that only
-- make sense for sea and inland waterway, where "on board" and "alongside ship"
-- have meaning. DPU replaced the older DAT — the difference from DAP is that
-- under DPU the seller unloads.
--
-- Deterministic ids so a second environment resolves the same rows.

INSERT INTO corporate.incoterms (id, code, name, status, metadata) VALUES
  -- Any mode of transport
  ('1c000000-0000-4a20-8000-000000000001', 'EXW', 'Ex Works',                      'active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000002', 'FCA', 'Free Carrier',                  'active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000003', 'CPT', 'Carriage Paid To',              'active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000004', 'CIP', 'Carriage and Insurance Paid To','active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000005', 'DAP', 'Delivered at Place',            'active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000006', 'DPU', 'Delivered at Place Unloaded',   'active', '{"transport":"any"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000007', 'DDP', 'Delivered Duty Paid',           'active', '{"transport":"any"}'::jsonb),
  -- Sea and inland waterway only
  ('1c000000-0000-4a20-8000-000000000008', 'FAS', 'Free Alongside Ship',           'active', '{"transport":"sea"}'::jsonb),
  ('1c000000-0000-4a20-8000-000000000009', 'FOB', 'Free on Board',                 'active', '{"transport":"sea"}'::jsonb),
  ('1c000000-0000-4a20-8000-00000000000a', 'CFR', 'Cost and Freight',              'active', '{"transport":"sea"}'::jsonb),
  ('1c000000-0000-4a20-8000-00000000000b', 'CIF', 'Cost, Insurance and Freight',   'active', '{"transport":"sea"}'::jsonb)
ON CONFLICT (id) DO NOTHING;
