use super::*;

#[test]
fn offered_capacity_and_admission_preserve_hosting_and_existing_reservations() {
    let gib = 1024_u64.pow(3);
    let policy = policy();
    let used = Resources {
        memory_bytes: 10 * gib,
        cpu_millis: 500,
        tasks: 128,
    };
    for (available_gib, expected_gib) in [(80, 15), (70, 5), (65, 0), (60, 0)] {
        let capacity = Capacity {
            total_memory: 100 * gib,
            available_memory: available_gib * gib,
            cpu_millis: 4000,
        };
        let free = available(&policy, capacity, used).unwrap();
        assert_eq!(
            free,
            Resources {
                memory_bytes: expected_gib * gib,
                cpu_millis: 500,
                tasks: 384
            }
        );
        if expected_gib == 0 {
            assert!(
                admit(
                    &policy,
                    capacity,
                    used,
                    Resources {
                        memory_bytes: 64 * 1024 * 1024,
                        cpu_millis: 1,
                        tasks: 1
                    }
                )
                .is_err()
            );
            continue;
        }
        assert!(admit(&policy, capacity, used, free).is_ok());
        for request in [
            Resources {
                memory_bytes: free.memory_bytes + 1,
                ..free
            },
            Resources {
                cpu_millis: free.cpu_millis + 1,
                ..free
            },
            Resources {
                tasks: free.tasks + 1,
                ..free
            },
        ] {
            assert!(admit(&policy, capacity, used, request).is_err());
        }
    }
}

#[test]
fn overdrawn_or_invalid_policy_cannot_offer_capacity() {
    let gib = 1024_u64.pow(3);
    let policy = policy();
    let capacity = Capacity {
        total_memory: 100 * gib,
        available_memory: 100 * gib,
        cpu_millis: 4000,
    };
    let overdrawn = Resources {
        memory_bytes: 30 * gib,
        cpu_millis: 2000,
        tasks: 600,
    };
    assert_eq!(
        available(&policy, capacity, overdrawn).unwrap(),
        Resources::default()
    );
    let invalid = Policy {
        protected_memory_percent: 49,
        ..policy
    };
    assert!(available(&invalid, capacity, Resources::default()).is_err());
}

fn policy() -> Policy {
    Policy {
        schema: 1,
        protected_memory_percent: 65,
        maximum_agent_memory_percent: 25,
        maximum_agent_cpu_percent: 25,
        maximum_agent_tasks: 512,
        agent_slice: "anima-agents.slice".into(),
        hosting_slice: "anima-hosting.slice".into(),
    }
}
