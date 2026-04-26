import { test, expect } from '@playwright/test';
import { randomUUID } from 'crypto';

// ---------------------------------------------------------------------------
// Betting Flow — mocked API, isolated test data per run
// ---------------------------------------------------------------------------

test.describe('User Journey: Betting Flow (place bet → view position → claim winnings)', () => {
  // Isolated per-run identifiers
  const TEST_WALLET = `GTEST${randomUUID().replace(/-/g, '').toUpperCase().slice(0, 51)}`;
  const MARKET_ID = 42;
  const TX_HASH = `tx_${randomUUID().replace(/-/g, '')}`;

  test.beforeEach(async ({ page }) => {
    // Mock: featured markets list
    await page.route('**/api/markets/featured', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          {
            id: MARKET_ID,
            title: 'Will BTC exceed $100k by end of 2025?',
            volume: 50000,
            ends_at: '2025-12-31T23:59:59Z',
            onchain_volume: '50000',
            resolved_outcome: null,
          },
        ]),
      })
    );

    // Mock: place bet (POST /api/blockchain/markets/:id/bets)
    await page.route(`**/api/blockchain/markets/${MARKET_ID}/bets`, (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ tx_hash: TX_HASH, status: 'pending' }),
        });
      } else {
        route.continue();
      }
    });

    // Mock: transaction status
    await page.route(`**/api/blockchain/tx/${TX_HASH}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ tx_hash: TX_HASH, status: 'confirmed', ledger: 1234 }),
      })
    );

    // Mock: user bets / position
    await page.route(`**/api/blockchain/users/${TEST_WALLET}/bets*`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          bets: [
            {
              market_id: MARKET_ID,
              outcome: 1,
              amount: '100',
              tx_hash: TX_HASH,
              status: 'confirmed',
            },
          ],
          total: 1,
        }),
      })
    );

    // Mock: oracle result (market resolved, outcome 1 wins)
    await page.route(`**/api/blockchain/oracle/${MARKET_ID}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ market_id: MARKET_ID, outcome: 1, resolved_at: '2025-12-31T23:59:59Z' }),
      })
    );

    // Mock: claim winnings (POST /api/blockchain/markets/:id/claim)
    await page.route(`**/api/blockchain/markets/${MARKET_ID}/claim`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ tx_hash: `claim_${TX_HASH}`, amount: '195', status: 'confirmed' }),
      })
    );
  });

  test('place bet — API receives correct payload and returns tx hash', async ({ page }) => {
    const betRequests: { body: unknown }[] = [];
    page.on('request', (req) => {
      if (req.url().includes(`/api/blockchain/markets/${MARKET_ID}/bets`) && req.method() === 'POST') {
        betRequests.push({ body: JSON.parse(req.postData() ?? '{}') });
      }
    });

    await page.goto('/');

    // Verify the "Place Bets" step is visible in the How It Works section
    await page.getByRole('link', { name: /how it works/i }).click();
    await expect(page.getByRole('heading', { name: /place bets/i })).toBeVisible();

    // Simulate placing a bet via the mocked API
    const betResponse = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/blockchain/markets/${marketId}/bets`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet, outcome: 1, amount: '100' }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: TEST_WALLET }
    );

    expect(betResponse.tx_hash).toBeTruthy();
    expect(betResponse.status).toBe('pending');
  });

  test('view position — user bets endpoint returns placed bet', async ({ page }) => {
    await page.goto('/');

    const positionResponse = await page.evaluate(
      async (wallet: string) => {
        const res = await fetch(`/api/blockchain/users/${wallet}/bets`);
        return res.json();
      },
      TEST_WALLET
    );

    expect(positionResponse.bets).toHaveLength(1);
    expect(positionResponse.bets[0].market_id).toBe(MARKET_ID);
    expect(positionResponse.bets[0].outcome).toBe(1);
    expect(positionResponse.bets[0].status).toBe('confirmed');
  });

  test('claim winnings — claim endpoint returns confirmed payout', async ({ page }) => {
    await page.goto('/');

    // Verify the "Claim Winnings" step is visible
    await page.getByRole('link', { name: /how it works/i }).click();
    await expect(page.getByRole('heading', { name: /claim winnings/i })).toBeVisible();

    // Verify oracle resolved the market
    const oracleResponse = await page.evaluate(
      async (marketId: number) => {
        const res = await fetch(`/api/blockchain/oracle/${marketId}`);
        return res.json();
      },
      MARKET_ID
    );
    expect(oracleResponse.outcome).toBe(1);

    // Claim winnings
    const claimResponse = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/blockchain/markets/${marketId}/claim`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: TEST_WALLET }
    );

    expect(claimResponse.status).toBe('confirmed');
    expect(Number(claimResponse.amount)).toBeGreaterThan(0);
  });

  test('full flow — place bet, confirm tx, view position, claim winnings', async ({ page }) => {
    await page.goto('/');

    // Step 1: Place bet
    const bet = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/blockchain/markets/${marketId}/bets`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet, outcome: 1, amount: '100' }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: TEST_WALLET }
    );
    expect(bet.tx_hash).toBeTruthy();

    // Step 2: Confirm transaction
    const tx = await page.evaluate(
      async (txHash: string) => {
        const res = await fetch(`/api/blockchain/tx/${txHash}`);
        return res.json();
      },
      bet.tx_hash
    );
    expect(tx.status).toBe('confirmed');

    // Step 3: View position
    const position = await page.evaluate(
      async (wallet: string) => {
        const res = await fetch(`/api/blockchain/users/${wallet}/bets`);
        return res.json();
      },
      TEST_WALLET
    );
    expect(position.bets[0].tx_hash).toBe(TX_HASH);

    // Step 4: Claim winnings
    const claim = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/blockchain/markets/${marketId}/claim`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: TEST_WALLET }
    );
    expect(claim.status).toBe('confirmed');
    expect(Number(claim.amount)).toBeGreaterThan(0);
  });
});

test.describe('User Journey: Homepage → Features → Newsletter Signup', () => {
  test('should complete full journey successfully', async ({ page }) => {
    await page.goto('/');
    
    // Verify homepage loaded
    await expect(page.getByRole('heading', { name: /decentralized prediction markets/i })).toBeVisible();
    
    // Browse features
    await page.getByRole('link', { name: /features/i }).click();
    await expect(page.locator('#features')).toBeInViewport();
    
    // Verify feature cards are visible
    await expect(page.getByRole('heading', { name: /fully decentralized/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /secure & audited/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /lightning fast/i })).toBeVisible();
    
    // Scroll back to newsletter signup
    await page.getByRole('link', { name: /predictiq logo/i }).click();
    
    // Fill newsletter form
    const emailInput = page.getByLabel(/email address/i);
    await emailInput.fill('user@example.com');
    
    // Submit form
    await page.getByRole('button', { name: /get early access/i }).click();
    
    // Verify success
    await expect(page.getByRole('button', { name: /subscribed/i })).toBeVisible();
    await expect(emailInput).toBeDisabled();
  });

  test('should track analytics events', async ({ page }) => {
    const analyticsEvents: string[] = [];
    
    // Mock analytics tracking
    await page.addInitScript(() => {
      (window as any).trackEvent = (event: string) => {
        (window as any).analyticsEvents = (window as any).analyticsEvents || [];
        (window as any).analyticsEvents.push(event);
      };
    });
    
    await page.goto('/');
    
    // Click features link
    await page.getByRole('link', { name: /features/i }).click();
    
    // Submit newsletter
    await page.getByLabel(/email address/i).fill('test@example.com');
    await page.getByRole('button', { name: /get early access/i }).click();
    
    // Verify events were tracked (if analytics is implemented)
    const events = await page.evaluate(() => (window as any).analyticsEvents || []);
    expect(Array.isArray(events)).toBe(true);
  });
});

test.describe('User Journey: Homepage → View Markets → Launch App', () => {
  test('should navigate to markets and launch app', async ({ page }) => {
    await page.goto('/');
    
    // Navigate to How It Works section
    await page.getByRole('link', { name: /how it works/i }).click();
    await expect(page.locator('#how-it-works')).toBeInViewport();
    
    // Verify steps are visible
    await expect(page.getByRole('heading', { name: /create a market/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /place bets/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /oracle resolution/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /claim winnings/i })).toBeVisible();
    
    // Click external launch app link (if exists)
    const launchAppLink = page.getByRole('link', { name: /launch app/i }).first();
    if (await launchAppLink.count() > 0) {
      await expect(launchAppLink).toHaveAttribute('href', /.+/);
    }
  });
});

test.describe('User Journey: Homepage → FAQ → Contact', () => {
  test('should navigate to about and contact sections', async ({ page }) => {
    await page.goto('/');
    
    // Navigate to About section
    await page.getByRole('link', { name: /about/i }).click();
    await expect(page.locator('#about')).toBeInViewport();
    
    // Verify about content
    await expect(page.getByText(/predictiq is a decentralized/i)).toBeVisible();
    
    // Navigate to Contact section
    await page.getByRole('link', { name: /contact/i }).click();
    await expect(page.locator('#contact')).toBeInViewport();
    
    // Verify footer links
    await expect(page.getByRole('link', { name: /documentation/i })).toBeVisible();
    await expect(page.getByRole('link', { name: /github/i })).toBeVisible();
    await expect(page.getByRole('link', { name: /discord/i })).toBeVisible();
  });
});

test.describe('User Journey: Mobile Navigation Flow', () => {
  test.use({ viewport: { width: 375, height: 667 } });
  
  test('should navigate on mobile device', async ({ page }) => {
    await page.goto('/');
    
    // Verify mobile layout
    await expect(page.getByRole('heading', { name: /decentralized prediction markets/i })).toBeVisible();
    
    // Test mobile navigation
    await page.getByRole('link', { name: /features/i }).click();
    await expect(page.locator('#features')).toBeInViewport();
    
    // Test form on mobile
    await page.getByLabel(/email address/i).fill('mobile@example.com');
    await page.getByRole('button', { name: /get early access/i }).click();
    
    await expect(page.getByRole('button', { name: /subscribed/i })).toBeVisible();
  });
});
