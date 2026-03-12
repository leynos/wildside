DROP INDEX IF EXISTS idx_overpass_enrichment_provenance_job_id;

ALTER TABLE overpass_enrichment_provenance
    DROP COLUMN IF EXISTS job_id;
