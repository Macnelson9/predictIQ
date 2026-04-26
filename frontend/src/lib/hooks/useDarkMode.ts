import { useState, useEffect } from 'react';

/**
 * Hook for managing dark mode preference
 * Respects system preference and allows manual toggle
 * Persists preference to localStorage
 */
export function useDarkMode() {
  const [isDarkMode, setIsDarkMode] = useState(false);
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    // Load preference from localStorage or system preference
    const stored = localStorage.getItem('darkMode');
    
    if (stored !== null) {
      setIsDarkMode(stored === 'true');
    } else {
      // Check system preference
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      setIsDarkMode(prefersDark);
    }
    
    setIsLoaded(true);
  }, []);

  useEffect(() => {
    if (!isLoaded) return;

    // Update localStorage
    localStorage.setItem('darkMode', String(isDarkMode));

    // Update document class
    if (isDarkMode) {
      document.documentElement.classList.add('dark-mode');
    } else {
      document.documentElement.classList.remove('dark-mode');
    }
  }, [isDarkMode, isLoaded]);

  const toggleDarkMode = () => {
    setIsDarkMode(!isDarkMode);
  };

  return {
    isDarkMode,
    toggleDarkMode,
    isLoaded,
  };
}
