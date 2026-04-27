import React from 'react';
import { useAsync } from '../lib/hooks/useAsync';
import { api } from '../lib/api/client';
import { LoadingSpinner } from './LoadingSpinner';
import { Skeleton } from './Skeleton';
import './Statistics.css';
import './Statistics.css';

interface StatisticsData {
  totalMarkets?: number;
  totalVolume?: number;
  activeUsers?: number;
  [key: string]: unknown;
}

export const Statistics: React.FC = () => {
  const { data, loading, error, execute } = useAsync<Record<string, unknown>>(
    () => api.getStatistics(),
    { immediate: true }
  );

  const handleRetry = () => {
    execute();
  };

  if (error) {
    return (
      <section className="statistics" aria-labelledby="statistics-heading">
        <h2 id="statistics-heading">Platform Statistics</h2>
        <div className="error-message" role="alert">
          <p>Failed to load statistics. Please try again.</p>
          <button onClick={handleRetry} className="retry-button">
            Retry
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="statistics" aria-labelledby="statistics-heading">
      <h2 id="statistics-heading">Platform Statistics</h2>
      <div className="stats-grid">
        <div className="stat-item">
          <h3>Total Markets</h3>
          {loading ? (
            <Skeleton width="4rem" height="2rem" aria-label="Loading total markets" />
          ) : (
            <p className="stat-value" aria-live="polite">
              {typeof data?.totalMarkets === 'number' ? data.totalMarkets : 'N/A'}
            </p>
          )}
        </div>
        <div className="stat-item">
          <h3>Total Volume</h3>
          {loading ? (
            <Skeleton width="6rem" height="2rem" aria-label="Loading total volume" />
          ) : (
            <p className="stat-value" aria-live="polite">
              ${typeof data?.totalVolume === 'number' ? data.totalVolume.toLocaleString() : 'N/A'}
            </p>
          )}
        </div>
        <div className="stat-item">
          <h3>Active Users</h3>
          {loading ? (
            <Skeleton width="5rem" height="2rem" aria-label="Loading active users" />
          ) : (
            <p className="stat-value" aria-live="polite">
              {typeof data?.activeUsers === 'number' ? data.activeUsers.toLocaleString() : 'N/A'}
            </p>
          )}
        </div>
      </div>
      {loading && (
        <div className="loading-overlay" aria-live="polite">
          <LoadingSpinner size="large" aria-label="Loading statistics data" />
          <p>Loading statistics...</p>
        </div>
      )}
    </section>
  );
};