/** @type {import('next').NextConfig} */
const nextConfig = {
  // Enable code splitting and optimization
  swcMinify: true,
  
  // Optimize bundle size
  webpack: (config, { isServer }) => {
    config.optimization = {
      ...config.optimization,
      splitChunks: {
        chunks: 'all',
        cacheGroups: {
          default: false,
          vendors: false,
          // Vendor chunk
          vendor: {
            filename: 'chunks/vendor.js',
            test: /node_modules/,
            priority: 10,
            reuseExistingChunk: true,
            enforce: true,
          },
          // Common chunk
          common: {
            minChunks: 2,
            priority: 5,
            reuseExistingChunk: true,
            filename: 'chunks/common.js',
          },
        },
      },
    };
    return config;
  },

  // Enable experimental features for better performance
  experimental: {
    optimizePackageImports: ['react', 'react-dom'],
  },

  // Compress responses
  compress: true,

  // Generate ETags for caching
  generateEtags: true,

  // Optimize images
  images: {
    formats: ['image/avif', 'image/webp'],
  },
};

module.exports = nextConfig;
