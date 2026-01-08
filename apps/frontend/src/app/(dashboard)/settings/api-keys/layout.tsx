import { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'API Keys | OMAR AI',
  description: 'Manage your API keys for programmatic access to OMAR AI',
  openGraph: {
    title: 'API Keys | OMAR AI',
    description: 'Manage your API keys for programmatic access to OMAR AI',
    type: 'website',
  },
};

export default async function APIKeysLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}
