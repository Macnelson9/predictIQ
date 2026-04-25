/**
 * Environment variable validation for frontend
 * Validates required environment variables at startup
 */

interface EnvConfig {
  apiUrl: string;
}

const REQUIRED_ENV_VARS = {
  NEXT_PUBLIC_API_URL: 'API endpoint URL',
} as const;

/**
 * Validates that all required environment variables are set
 * @throws Error if any required environment variable is missing
 */
export function validateEnvironment(): EnvConfig {
  const missing: string[] = [];

  for (const [key, description] of Object.entries(REQUIRED_ENV_VARS)) {
    const value = process.env[key];
    if (!value || value.trim() === '') {
      missing.push(`${key} (${description})`);
    }
  }

  if (missing.length > 0) {
    const errorMessage = `Missing required environment variables:\n${missing.map(v => `  - ${v}`).join('\n')}`;
    
    // In development, log warning; in production, throw error
    if (process.env.NODE_ENV === 'production') {
      throw new Error(errorMessage);
    } else {
      console.warn('⚠️  Environment validation warning:\n' + errorMessage);
    }
  }

  return {
    apiUrl: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001',
  };
}

/**
 * Get validated environment configuration
 */
export function getEnvConfig(): EnvConfig {
  return {
    apiUrl: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:3001',
  };
}

// Validate environment on module load
if (typeof window === 'undefined') {
  // Server-side validation
  try {
    validateEnvironment();
  } catch (error) {
    console.error('Environment validation failed:', error);
    if (process.env.NODE_ENV === 'production') {
      throw error;
    }
  }
}
