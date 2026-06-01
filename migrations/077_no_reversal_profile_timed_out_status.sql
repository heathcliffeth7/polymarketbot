ALTER TABLE no_reversal_adverse_profiles
  DROP CONSTRAINT IF EXISTS chk_no_reversal_adverse_profiles_status;

ALTER TABLE no_reversal_adverse_profiles
  ADD CONSTRAINT chk_no_reversal_adverse_profiles_status
    CHECK (status IN ('ready', 'insufficient', 'error', 'stale', 'timed_out'));
