import NodeStatus from '@/components/NodeStatus';

export default function NodeStatusPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <div className="mb-8">
        <h1 className="text-3xl font-bold text-gray-900">Node Status</h1>
        <p className="mt-2 text-gray-600">
          Monitor the health and performance of the TWINS blockchain node
        </p>
      </div>
      
      <NodeStatus />
    </div>
  );
} 