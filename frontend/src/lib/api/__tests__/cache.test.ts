import { apiCache, CACHE_TTL } from '../cache';

describe('apiCache', () => {
  beforeEach(() => {
    apiCache.clear();
  });

  describe('get and set', () => {
    it('should store and retrieve cached data', () => {
      const key = 'test-key';
      const data = { id: 1, name: 'test' };

      apiCache.set(key, data, CACHE_TTL.SHORT);
      const result = apiCache.get(key);

      expect(result).toEqual(data);
    });

    it('should return null for non-existent keys', () => {
      const result = apiCache.get('non-existent');
      expect(result).toBeNull();
    });

    it('should return null for expired entries', () => {
      const key = 'test-key';
      const data = { id: 1 };

      apiCache.set(key, data, 1); // 1ms TTL
      
      // Wait for expiration
      return new Promise(resolve => {
        setTimeout(() => {
          const result = apiCache.get(key);
          expect(result).toBeNull();
          resolve(undefined);
        }, 10);
      });
    });
  });

  describe('invalidateByPattern', () => {
    it('should invalidate cache by string pattern', () => {
      apiCache.set('api/users/1', { id: 1 }, CACHE_TTL.SHORT);
      apiCache.set('api/users/2', { id: 2 }, CACHE_TTL.SHORT);
      apiCache.set('api/posts/1', { id: 1 }, CACHE_TTL.SHORT);

      apiCache.invalidateByPattern('api/users');

      expect(apiCache.get('api/users/1')).toBeNull();
      expect(apiCache.get('api/users/2')).toBeNull();
      expect(apiCache.get('api/posts/1')).not.toBeNull();
    });

    it('should invalidate cache by regex pattern', () => {
      apiCache.set('api/users/1', { id: 1 }, CACHE_TTL.SHORT);
      apiCache.set('api/posts/1', { id: 1 }, CACHE_TTL.SHORT);

      apiCache.invalidateByPattern(/users/);

      expect(apiCache.get('api/users/1')).toBeNull();
      expect(apiCache.get('api/posts/1')).not.toBeNull();
    });
  });

  describe('clear', () => {
    it('should clear all cache entries', () => {
      apiCache.set('key1', { data: 1 }, CACHE_TTL.SHORT);
      apiCache.set('key2', { data: 2 }, CACHE_TTL.SHORT);

      apiCache.clear();

      expect(apiCache.get('key1')).toBeNull();
      expect(apiCache.get('key2')).toBeNull();
    });
  });

  describe('TTL constants', () => {
    it('should have correct TTL values', () => {
      expect(CACHE_TTL.SHORT).toBe(60 * 1000);
      expect(CACHE_TTL.MEDIUM).toBe(5 * 60 * 1000);
      expect(CACHE_TTL.LONG).toBe(30 * 60 * 1000);
    });
  });
});
