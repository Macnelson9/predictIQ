# Frontend Internationalization (i18n) Guide

## Overview

PredictIQ frontend supports multiple languages through a simple i18n system. Currently supported locales:
- **en** - English (default)
- **es** - Spanish
- **fr** - French
- **de** - German

## Adding Translations

### 1. Add Translation Keys

Edit `frontend/src/lib/i18n.ts` and add your translation strings to the `translations` object:

```typescript
const translations: LocaleData = {
  en: {
    mySection: {
      myKey: 'English text',
    },
  },
};
```

### 2. Use in Components

Import and use the `useI18n` hook:

```typescript
import { useI18n } from '../lib/hooks/useI18n';

export function MyComponent() {
  const { t } = useI18n();
  
  return <h1>{t('mySection.myKey')}</h1>;
}
```

### 3. Language Selector

Users can change language via the language selector in the header. The selection is persisted to localStorage.

## Adding a New Language

1. Add translations to `frontend/src/lib/i18n.ts`:

```typescript
const translations: LocaleData = {
  en: { /* ... */ },
  pt: {  // Portuguese
    nav: {
      features: 'Recursos',
      // ... add all keys
    },
  },
};
```

2. The new locale will automatically appear in the language selector.

## Translation Keys Structure

Keys follow a hierarchical dot-notation pattern:
- `nav.features` - Navigation features link
- `hero.title` - Hero section title
- `features.decentralized.title` - Feature card title

## Testing

Run i18n tests:

```bash
npm run test -- i18n.test.ts
```

## Best Practices

1. **Keep keys organized** - Group related translations by section
2. **Use descriptive names** - Make key names self-documenting
3. **Avoid hardcoded strings** - Always use `t()` for user-facing text
4. **Provide defaults** - Use `t('key', 'default')` for fallback values
5. **Test all locales** - Verify translations work in all supported languages

## Locale Persistence

User's language preference is automatically saved to localStorage and restored on next visit.

## Fallback Behavior

- If a translation key is missing, the key itself is returned
- If a default value is provided, it's returned instead
- English is the default locale if no preference is stored
