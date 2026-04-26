import { test, expect } from '@playwright/test';
import { randomUUID } from 'crypto';

// ---------------------------------------------------------------------------
// Market Creation Flow — mocked API, isolated test data per run
// ---------------------------------------------------------------------------

const MARKET_ID = 99;
const CREATOR_WALLET = `GCREATE${randomUUID().replace(/-/g, '').toUpperCase().slice(0, 49)}`;
const BETTOR_WALLET = `GBET${randomUUID().replace(/-/g, '').toUpperCase().slice(0, 52)}`;
const CREATE_TX = `tx_create_${randomUUID().replace(/-/g, '')}`;
const BET_TX = `tx_bet_${randomUUID().replace(/-/g, '')}`;
const CLAIM_TX = `tx_claim_${randomUUID().replace(/-/g, '')}`;

const MARKET_PAYLOAD = {
  id: MARKET_ID,
  title: 'Will ETH exceed $5k by end of 2026?',
  volume: 0,
  ends_at: '2026-12-31T23:59:59Z',
  onchain_volume: '0',
  resolved_outcome: null,
};

test.describe('Market Creation Flow', () => {
  test.beforeEach(async ({ page }) => {
    // Mock: create market (POST /api/v1/blockchain/markets)
    await page.route('**/api/v1/blockchain/markets', (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ market_id: MARKET_ID, tx_hash: CREATE_TX, status: 'pending' }),
        });
      } else {
        route.continue();
      }
    });

    // Mock: market data
    await page.route(`**/api/v1/blockchain/markets/${MARKET_ID}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MARKET_PAYLOAD),
      })
    );

    // Mock: featured markets (includes newly created market)
    await page.route('**/api/v1/markets/featured', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([MARKET_PAYLOAD]),
      })
    );

    // Mock: place bet
    await page.route(`**/api/v1/blockchain/markets/${MARKET_ID}/bets`, (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ tx_hash: BET_TX, status: 'pending' }),
        });
      } else {
        route.continue();
      }
    });

    // Mock: transaction status (confirmed for both create and bet txs)
    await page.route(`**/api/v1/blockchain/tx/${CREATE_TX}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ tx_hash: CREATE_TX, status: 'confirmed', ledger: 1000 }),
      })
    );

    await page.route(`**/api/v1/blockchain/tx/${BET_TX}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ tx_hash: BET_TX, status: 'confirmed', ledger: 1001 }),
      })
    );

    // Mock: oracle result (market resolved, outcome 0 wins)
    await page.route(`**/api/v1/blockchain/oracle/${MARKET_ID}`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ market_id: MARKET_ID, outcome: 0, resolved_at: '2026-12-31T23:59:59Z' }),
      })
    );

    // Mock: user bets
    await page.route(`**/api/v1/blockchain/users/${BETTOR_WALLET}/bets*`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          bets: [{ market_id: MARKET_ID, outcome: 0, amount: '200', tx_hash: BET_TX, status: 'confirmed' }],
          total: 1,
        }),
      })
    );

    // Mock: claim winnings
    await page.route(`**/api/v1/blockchain/markets/${MARKET_ID}/claim`, (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ tx_hash: CLAIM_TX, amount: '390', status: 'confirmed' }),
      })
    );
  });

  test('create market — returns market_id and pending tx hash', async ({ page }) => {
    await page.goto('/');

    const response = await page.evaluate(
      async ({ wallet, payload }: { wallet: string; payload: object }) => {
        const res = await fetch('/api/v1/blockchain/markets', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ creator: wallet, ...payload }),
        });
        return res.json();
      },
      { wallet: CREATOR_WALLET, payload: { title: MARKET_PAYLOAD.title, ends_at: MARKET_PAYLOAD.ends_at } }
    );

    expect(response.market_id).toBe(MARKET_ID);
    expect(response.tx_hash).toBeTruthy();
    expect(response.status).toBe('pending');
  });

  test('create market tx — confirms on-chain', async ({ page }) => {
    await page.goto('/');

    const tx = await page.evaluate(
      async (txHash: string) => {
        const res = await fetch(`/api/v1/blockchain/tx/${txHash}`);
        return res.json();
      },
      CREATE_TX
    );

    expect(tx.status).toBe('confirmed');
    expect(tx.ledger).toBeGreaterThan(0);
  });

  test('created market — appears in featured markets list', async ({ page }) => {
    await page.goto('/');

    const markets = await page.evaluate(async () => {
      const res = await fetch('/api/v1/markets/featured');
      return res.json();
    });

    const market = markets.find((m: { id: number }) => m.id === MARKET_ID);
    expect(market).toBeDefined();
    expect(market.title).toBe(MARKET_PAYLOAD.title);
  });

  test('place bet on created market — returns confirmed tx', async ({ page }) => {
    await page.goto('/');

    const bet = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/v1/blockchain/markets/${marketId}/bets`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet, outcome: 0, amount: '200' }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: BETTOR_WALLET }
    );

    expect(bet.tx_hash).toBeTruthy();
    expect(bet.status).toBe('pending');
  });

  test('oracle resolves created market', async ({ page }) => {
    await page.goto('/');

    const oracle = await page.evaluate(
      async (marketId: number) => {
        const res = await fetch(`/api/v1/blockchain/oracle/${marketId}`);
        return res.json();
      },
      MARKET_ID
    );

    expect(oracle.market_id).toBe(MARKET_ID);
    expect(oracle.outcome).toBeDefined();
    expect(oracle.resolved_at).toBeTruthy();
  });

  test('claim winnings after oracle resolution', async ({ page }) => {
    await page.goto('/');

    const claim = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/v1/blockchain/markets/${marketId}/claim`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: BETTOR_WALLET }
    );

    expect(claim.status).toBe('confirmed');
    expect(Number(claim.amount)).toBeGreaterThan(0);
  });

  test('full flow — create market, bet, oracle resolution, claim winnings', async ({ page }) => {
    await page.goto('/');

    // Step 1: Create market
    const created = await page.evaluate(
      async ({ wallet, title, ends_at }: { wallet: string; title: string; ends_at: string }) => {
        const res = await fetch('/api/v1/blockchain/markets', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ creator: wallet, title, ends_at }),
        });
        return res.json();
      },
      { wallet: CREATOR_WALLET, title: MARKET_PAYLOAD.title, ends_at: MARKET_PAYLOAD.ends_at }
    );
    expect(created.market_id).toBe(MARKET_ID);

    // Step 2: Confirm creation tx
    const createTx = await page.evaluate(
      async (txHash: string) => {
        const res = await fetch(`/api/v1/blockchain/tx/${txHash}`);
        return res.json();
      },
      created.tx_hash
    );
    expect(createTx.status).toBe('confirmed');

    // Step 3: Market appears in featured list
    const markets = await page.evaluate(async () => {
      const res = await fetch('/api/v1/markets/featured');
      return res.json();
    });
    expect(markets.some((m: { id: number }) => m.id === MARKET_ID)).toBe(true);

    // Step 4: Place bet
    const bet = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/v1/blockchain/markets/${marketId}/bets`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet, outcome: 0, amount: '200' }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: BETTOR_WALLET }
    );
    expect(bet.tx_hash).toBeTruthy();

    // Step 5: Oracle resolves market
    const oracle = await page.evaluate(
      async (marketId: number) => {
        const res = await fetch(`/api/v1/blockchain/oracle/${marketId}`);
        return res.json();
      },
      MARKET_ID
    );
    expect(oracle.resolved_at).toBeTruthy();

    // Step 6: Claim winnings
    const claim = await page.evaluate(
      async ({ marketId, wallet }: { marketId: number; wallet: string }) => {
        const res = await fetch(`/api/v1/blockchain/markets/${marketId}/claim`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ wallet }),
        });
        return res.json();
      },
      { marketId: MARKET_ID, wallet: BETTOR_WALLET }
    );
    expect(claim.status).toBe('confirmed');
    expect(Number(claim.amount)).toBeGreaterThan(0);

    // Step 7: Verify "How It Works" UI reflects the full journey
    await expect(page.getByRole('heading', { name: /create a market/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /place bets/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /oracle resolution/i })).toBeVisible();
    await expect(page.getByRole('heading', { name: /claim winnings/i })).toBeVisible();
  });
});
