import { createHmac } from "crypto";
import {
  authenticate,
  AuthError,
  ApiKeyAuthConfig,
  JwtAuthConfig,
  TTSService,
  VOICES,
} from "../TTSService";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const API_KEY_CONFIG: ApiKeyAuthConfig = { type: "apikey", keys: ["key-abc", "key-xyz"] };
const JWT_SECRET = "test-secret";
const JWT_CONFIG: JwtAuthConfig = { type: "jwt", secret: JWT_SECRET };

function makeJwt(payload: object, secret: string): string {
  const header = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString("base64url");
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const sig = createHmac("sha256", secret).update(`${header}.${body}`).digest("base64url");
  return `${header}.${body}.${sig}`;
}

const VALID_JWT = makeJwt({ sub: "user1" }, JWT_SECRET);
const EXPIRED_JWT = makeJwt({ sub: "user1", exp: Math.floor(Date.now() / 1000) - 60 }, JWT_SECRET);

// ---------------------------------------------------------------------------
// authenticate() — API key
// ---------------------------------------------------------------------------

describe("authenticate — apikey", () => {
  it("accepts a valid key", () => {
    expect(() => authenticate("key-abc", API_KEY_CONFIG)).not.toThrow();
  });

  it("throws AuthError for an invalid key", () => {
    expect(() => authenticate("bad-key", API_KEY_CONFIG)).toThrow(AuthError);
  });

  it("throws AuthError when credential is missing", () => {
    expect(() => authenticate(undefined, API_KEY_CONFIG)).toThrow(AuthError);
  });

  it("sets statusCode 401", () => {
    try {
      authenticate("bad", API_KEY_CONFIG);
    } catch (e) {
      expect((e as AuthError).statusCode).toBe(401);
    }
  });
});

// ---------------------------------------------------------------------------
// authenticate() — JWT
// ---------------------------------------------------------------------------

describe("authenticate — jwt", () => {
  it("accepts a valid JWT", () => {
    expect(() => authenticate(VALID_JWT, JWT_CONFIG)).not.toThrow();
  });

  it("throws AuthError for a wrong signature", () => {
    const tampered = VALID_JWT.slice(0, -3) + "xxx";
    expect(() => authenticate(tampered, JWT_CONFIG)).toThrow(AuthError);
  });

  it("throws AuthError for an expired JWT", () => {
    expect(() => authenticate(EXPIRED_JWT, JWT_CONFIG)).toThrow(AuthError);
  });

  it("throws AuthError for a malformed token", () => {
    expect(() => authenticate("not.a.jwt.at.all", JWT_CONFIG)).toThrow(AuthError);
  });

  it("throws AuthError when credential is missing", () => {
    expect(() => authenticate(undefined, JWT_CONFIG)).toThrow(AuthError);
  });
});

// ---------------------------------------------------------------------------
// TTSService — auth integration (no real provider calls needed)
// ---------------------------------------------------------------------------

const VOICE = VOICES["el-rachel-en"];

function makeService(authConfig?: ApiKeyAuthConfig | JwtAuthConfig) {
  return new TTSService({
    provider: "elevenlabs",
    elevenlabs: { apiKey: "el-key" },
    outputDir: "/tmp/tts-test",
    ...(authConfig ? { auth: authConfig } : {}),
  });
}

describe("TTSService.enqueue — auth", () => {
  it("allows calls when auth is not configured", () => {
    const svc = makeService();
    expect(() => svc.enqueue("hello", VOICE)).not.toThrow();
  });

  it("allows calls with a valid API key", () => {
    const svc = makeService(API_KEY_CONFIG);
    expect(() => svc.enqueue("hello", VOICE, undefined, "key-abc")).not.toThrow();
  });

  it("throws AuthError with an invalid API key", () => {
    const svc = makeService(API_KEY_CONFIG);
    expect(() => svc.enqueue("hello", VOICE, undefined, "wrong")).toThrow(AuthError);
  });

  it("throws AuthError when no credential is provided but auth is configured", () => {
    const svc = makeService(API_KEY_CONFIG);
    expect(() => svc.enqueue("hello", VOICE)).toThrow(AuthError);
  });

  it("allows calls with a valid JWT", () => {
    const svc = makeService(JWT_CONFIG);
    expect(() => svc.enqueue("hello", VOICE, undefined, VALID_JWT)).not.toThrow();
  });

  it("throws AuthError with an invalid JWT", () => {
    const svc = makeService(JWT_CONFIG);
    expect(() => svc.enqueue("hello", VOICE, undefined, "bad.jwt.token")).toThrow(AuthError);
  });
});
