# Copyright 2024 Amazon.com, Inc. or its affiliates. All Rights Reserved.
# SPDX-License-Identifier: Apache-2.0
"""A test that ensures that clawdbox builds with GDB feature enabled at integration time."""

import host_tools.cargo_build


def test_gdb_compiles():
    """Checks that clawdbox compiles with GDB enabled"""

    host_tools.cargo_build.build_gdb()
