#!/bin/bash
#=============================================================================#
# This script is what will be executed by the client instance
#
# > You can assume it will be triggered inside the proper path
# > IMPORTANT: The trap below is required for the client to capture
# > the exit code of this script. Without it, the client cannot
# > determine if the job succeeded or failed.
#=============================================================================#

# Required: Capture exit code for the orchestrator client
trap 'echo "$?" > .orchestrator.exit' EXIT

gdock score --receptor 2oob_A.pdb --ligand 2oob_B.pdb &>gdock.log

# The client will decide if the execution failed or not based on the exit
#  status, so make sure your application has proper exit status 0 for success
#  and exit status != 0 for failures

#=============================================================================#
