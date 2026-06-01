import assert from 'node:assert/strict';
import test from 'node:test';

import {
  applyLiveGapCollectorFormDefaults,
  LIVE_GAP_COLLECTOR_MODE,
  normalizeLiveGapCollectorBuildConfig,
} from './live-gap-collector';

test('live gap collector near-miss notification defaults and normalizes', () => {
  const fields: Record<string, string> = {
    mode: LIVE_GAP_COLLECTOR_MODE,
  };

  applyLiveGapCollectorFormDefaults(fields);
  assert.equal(fields.notifyOnLiveGapAdaptiveLowGapNearMissChange, 'true');

  const config: Record<string, unknown> = {
    mode: LIVE_GAP_COLLECTOR_MODE,
    notifyOnLiveGapAdaptiveLowGapNearMissChange: 'true',
  };
  assert.equal(normalizeLiveGapCollectorBuildConfig(config), true);
  assert.equal(config.notifyOnLiveGapAdaptiveLowGapNearMissChange, true);

  config.notifyOnLiveGapAdaptiveLowGapNearMissChange = false;
  assert.equal(normalizeLiveGapCollectorBuildConfig(config), true);
  assert.equal(config.notifyOnLiveGapAdaptiveLowGapNearMissChange, false);
});

test('live gap collector local path only mode normalizes guard sources', () => {
  const fields: Record<string, string> = {
    mode: LIVE_GAP_COLLECTOR_MODE,
    noReversalDecisionMode: 'local_path_only',
  };

  applyLiveGapCollectorFormDefaults(fields);
  assert.equal(fields.noReversalDecisionMode, 'local_path_only');
  assert.equal(fields.noReversalLocalPathGateMode, 'clean_floor');

  const config: Record<string, unknown> = {
    mode: LIVE_GAP_COLLECTOR_MODE,
    noReversalEntryGuardEnabled: true,
    noReversalDecisionMode: fields.noReversalDecisionMode,
    noReversalPrecomputedProfilesEnabled: true,
    noReversalAllowColdProfileQuery: true,
    noReversalLocalPathFallbackEnabled: false,
    noReversalUseLocalPathFallbackOnMissingProfile: false,
    noReversalBlockIfProfileMissingAndLocalPathInsufficient: false,
    noReversalLocalPathGateMode: 'fresh_floor_touch',
    noReversalLocalPathFreshRetraceWindowMs: '10000',
    noReversalLocalPathFreshMaxDropUsd: '5',
    noReversalLocalPathFreshMinHistoryMs: '1000',
  };

  assert.equal(normalizeLiveGapCollectorBuildConfig(config), true);
  assert.equal(config.noReversalDecisionMode, 'local_path_only');
  assert.equal(config.noReversalPrecomputedProfilesEnabled, false);
  assert.equal(config.noReversalAllowColdProfileQuery, false);
  assert.equal(config.noReversalLocalPathFallbackEnabled, true);
  assert.equal(config.noReversalUseLocalPathFallbackOnMissingProfile, true);
  assert.equal(config.noReversalBlockIfProfileMissingAndLocalPathInsufficient, true);
  assert.equal(config.noReversalLocalPathGateMode, 'fresh_floor_touch');
  assert.equal(config.noReversalLocalPathFreshRetraceWindowMs, 10000);
  assert.equal(config.noReversalLocalPathFreshMaxDropUsd, 5);
  assert.equal(config.noReversalLocalPathFreshMinHistoryMs, 1000);
});
