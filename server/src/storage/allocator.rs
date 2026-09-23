use super::disk::DiskStatus;

#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    MostFreeSpace,
    LowestUsage,
}

#[derive(Debug, Clone)]
pub struct DiskAllocator {
    strategy: AllocationStrategy,
}

impl Default for DiskAllocator {
    fn default() -> Self {
        Self {
            strategy: AllocationStrategy::MostFreeSpace,
        }
    }
}

impl DiskAllocator {
    pub fn new(strategy: AllocationStrategy) -> Self {
        Self { strategy }
    }

    pub fn choose<'a>(
        &self,
        disks: &'a [DiskStatus],
        required_bytes: u64,
    ) -> Option<&'a DiskStatus> {
        let eligible = disks.iter().filter(|disk| {
            disk.enabled
                && disk.mounted
                && disk.writable
                && disk.allocatable_bytes >= required_bytes
        });

        match self.strategy {
            AllocationStrategy::MostFreeSpace => eligible.max_by_key(|disk| disk.allocatable_bytes),
            AllocationStrategy::LowestUsage => {
                eligible.min_by(|left, right| left.usage_percent.total_cmp(&right.usage_percent))
            }
        }
    }
}
