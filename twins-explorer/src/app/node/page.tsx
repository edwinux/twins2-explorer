"use client";

import { useState } from 'react';
import NodeStatus from '@/components/NodeStatus';

async function resetSync() {
  const confirmed = window.confirm(
    'Resetting sync will delete all local blocks and restart the sync process. Continue?' 
  );
  if (!confirmed) return;

  await fetch('/api/v1/reset-sync', { method: 'POST' });
}

export default function NodeStatusPage() {
  const [resetting, setResetting] = useState(false);

  const handleReset = async () => {
    setResetting(true);
    try {
      await resetSync();
    } finally {
      setResetting(false);
    }
  };
  return (
    <div className="container mx-auto px-4 py-8">
      <div className="mb-8">
        <h1 className="text-3xl font-bold text-gray-900">Node Status</h1>
        <p className="mt-2 text-gray-600">
          Monitor the health and performance of the TWINS blockchain node
        </p>
      </div>

      <NodeStatus />
      <button
        onClick={handleReset}
        disabled={resetting}
        className="mt-6 rounded bg-red-600 px-4 py-2 text-sm font-medium text-white disabled:opacity-60"
      >
        {resetting ? 'Resetting…' : 'Reset Sync'}
      </button>
    </div>
  );
}
