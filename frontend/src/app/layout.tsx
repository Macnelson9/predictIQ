import { ErrorBoundary } from '../components/ErrorBoundary';

export const metadata = { title: 'PredictIQ' };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  const { isDarkMode } = useDarkMode();

  return (
    <html lang="en">
      <body>
        <ErrorBoundary section="main">
          {children}
        </ErrorBoundary>
      </body>
    </html>
  );
}
