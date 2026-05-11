const SWEEP_INTERVAL_MS = 120_000; // 2 dakika

export async function register() {
  if (process.env.NEXT_RUNTIME !== 'nodejs') {
    return;
  }

  let running = false;

  setInterval(async () => {
    if (running) return;
    running = true;
    try {
      const { runClaimSweep } = await import('@/lib/claim-sweep');
      const { getServiceStatus } = await import('@/lib/systemctl');
      const serviceStatus = await getServiceStatus();
      const systemUser = { userId: 1, username: 'auto-sweep' };
      const result = await runClaimSweep(systemUser, {
        serviceActive: serviceStatus.serviceActive,
      });
      if (result.queuedNewCount > 0 || result.rearmedCount > 0) {
        console.log(
          `[AUTO_SWEEP] queued=${result.queuedNewCount} rearmed=${result.rearmedCount} already=${result.alreadyTrackedCount} eligible=${result.eligibleCount} total_usdc=${result.eligibleTotalUsdc}`
        );
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (!message.includes('no_eligible_claims')) {
        console.error('[AUTO_SWEEP] Error:', message);
      }
    } finally {
      running = false;
    }
  }, SWEEP_INTERVAL_MS);

  console.log('[AUTO_SWEEP] Claim sweep scheduler started (interval: 120s)');
}
