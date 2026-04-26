'use client';
import dynamic from 'next/dynamic';
import { Suspense } from 'react';

// Dynamic import with code splitting
const LandingPage = dynamic(() => import('../components/LandingPage').then(mod => ({ default: mod.LandingPage })), {
  loading: () => <div role="status" aria-live="polite">Loading...</div>,
  ssr: true,
});

export default function Home() {
  return (
    <Suspense fallback={<div role="status" aria-live="polite">Loading page...</div>}>
      <LandingPage />
    </Suspense>
  );
}
