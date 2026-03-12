ALTER TABLE overpass_enrichment_provenance
    ADD COLUMN IF NOT EXISTS job_id UUID;

UPDATE overpass_enrichment_provenance
SET job_id = gen_random_uuid()
WHERE job_id IS NULL;

ALTER TABLE overpass_enrichment_provenance
    ALTER COLUMN job_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_overpass_enrichment_provenance_job_id
    ON overpass_enrichment_provenance (job_id);
