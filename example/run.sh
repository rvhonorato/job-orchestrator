#!/bin/bash
#=============================================================================#
# This script is what will be executed by the client instance
#
# > You can assume it will be triggered inside the proper path
#=============================================================================#

gdock score --receptor 2oob_A.pdb --ligand 2oob_B.pdb &>gdock.log

# The client will decide if the execution failed or not based on the exit
#  status, so make sure your application has proper exit status 0 for success
#  and exit status != 0 for failures

#=============================================================================#
