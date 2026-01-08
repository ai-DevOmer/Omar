import { Metadata } from 'next';
import { redirect } from 'next/navigation';

export const metadata: Metadata = {
  title: 'Worker Conversation | OMAR AI',
  description: 'Interactive Worker conversation powered by OMAR AI',
  openGraph: {
    title: 'Worker Conversation | OMAR AI',
    description: 'Interactive Worker conversation powered by OMAR AI',
    type: 'website',
  },
};

export default async function AgentsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <>{children}</>;
}
